import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { StellarFlow } from "../target/types/stellar_flow";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  createMint,
  createAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
  getAccount,
} from "@solana/spl-token";
import { BankrunProvider, startAnchor } from "anchor-bankrun";

const IDL = require("../target/idl/stellar_flow.json");
const PROGRAM_ID = new PublicKey("SFLWxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

describe("StellarFlow - Decentralized Lending Protocol", () => {
  let context: any;
  let provider: BankrunProvider;
  let program: Program<StellarFlow>;

  // Keypairs
  let authority: Keypair;
  let user: Keypair;
  let liquidator: Keypair;

  // Token mints
  let collateralMint: PublicKey; // e.g. SOL-like token, price = $100
  let borrowMint: PublicKey;     // e.g. USDC-like token, price = $1

  // Token accounts
  let userCollateralAccount: PublicKey;
  let userBorrowAccount: PublicKey;
  let liquidatorCollateralAccount: PublicKey;
  let liquidatorBorrowAccount: PublicKey;

  // PDAs
  let marketPda: PublicKey;
  let collateralReservePda: PublicKey;
  let borrowReservePda: PublicKey;
  let collateralVaultPda: PublicKey;
  let borrowVaultPda: PublicKey;
  let userCollateralPositionPda: PublicKey;
  let userBorrowPositionPda: PublicKey;

  before(async () => {
    authority = Keypair.generate();
    user = Keypair.generate();
    liquidator = Keypair.generate();

    context = await startAnchor(
      "",
      [{ name: "stellar_flow", programId: PROGRAM_ID }],
      []
    );
    provider = new BankrunProvider(context);
    program = new Program<StellarFlow>(IDL, provider);

    // Airdrop SOL to accounts
    await provider.context.banksClient.requestAirdrop(
      authority.publicKey,
      10_000_000_000
    );
    await provider.context.banksClient.requestAirdrop(
      user.publicKey,
      10_000_000_000
    );
    await provider.context.banksClient.requestAirdrop(
      liquidator.publicKey,
      10_000_000_000
    );

    // Create token mints
    collateralMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      6
    );

    borrowMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      6
    );

    // Create user token accounts
    userCollateralAccount = await createAccount(
      provider.connection,
      user,
      collateralMint,
      user.publicKey
    );

    userBorrowAccount = await createAccount(
      provider.connection,
      user,
      borrowMint,
      user.publicKey
    );

    // Create liquidator token accounts
    liquidatorCollateralAccount = await createAccount(
      provider.connection,
      liquidator,
      collateralMint,
      liquidator.publicKey
    );

    liquidatorBorrowAccount = await createAccount(
      provider.connection,
      liquidator,
      borrowMint,
      liquidator.publicKey
    );

    // Mint tokens to user and liquidator
    await mintTo(
      provider.connection,
      authority,
      collateralMint,
      userCollateralAccount,
      authority,
      1_000_000_000 // 1000 tokens
    );

    await mintTo(
      provider.connection,
      authority,
      borrowMint,
      userBorrowAccount,
      authority,
      1_000_000_000
    );

    await mintTo(
      provider.connection,
      authority,
      borrowMint,
      liquidatorBorrowAccount,
      authority,
      1_000_000_000
    );

    // Derive PDAs
    [marketPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("market"), authority.publicKey.toBuffer()],
      PROGRAM_ID
    );

    [collateralReservePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reserve"), marketPda.toBuffer(), collateralMint.toBuffer()],
      PROGRAM_ID
    );

    [borrowReservePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("reserve"), marketPda.toBuffer(), borrowMint.toBuffer()],
      PROGRAM_ID
    );

    [collateralVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), collateralReservePda.toBuffer()],
      PROGRAM_ID
    );

    [borrowVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), borrowReservePda.toBuffer()],
      PROGRAM_ID
    );

    [userCollateralPositionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("position"), collateralReservePda.toBuffer(), user.publicKey.toBuffer()],
      PROGRAM_ID
    );

    [userBorrowPositionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("position"), borrowReservePda.toBuffer(), user.publicKey.toBuffer()],
      PROGRAM_ID
    );
  });

  // ─────────────────────────────────────────────
  // INITIALIZE MARKET
  // ─────────────────────────────────────────────
  describe("initialize_market", () => {
    it("initializes the lending market", async () => {
      await program.methods
        .initializeMarket(new BN(50)) // 0.5% protocol fee
        .accountsPartial({
          market: marketPda,
          authority: authority.publicKey,
          treasury: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([authority])
        .rpc();

      const market = await program.account.market.fetch(marketPda);
      console.log("Market:", market);

      anchor.assert.strictEqual(
        market.authority.toBase58(),
        authority.publicKey.toBase58()
      );
      anchor.assert.strictEqual(market.isPaused, false);
      anchor.assert.strictEqual(market.protocolFeeBps.toNumber(), 50);
      anchor.assert.strictEqual(market.reserveCount.toNumber(), 0);
    });
  });

  // ─────────────────────────────────────────────
  // INITIALIZE RESERVES
  // ─────────────────────────────────────────────
  describe("initialize_reserve", () => {
    it("initializes collateral reserve (price = $100)", async () => {
      await program.methods
        .initializeReserve(
          new BN(7500),   // ltv 75%
          new BN(8000),   // liquidation threshold 80%
          new BN(500),    // liquidation bonus 5%
          new BN(8000),   // optimal utilization 80%
          new BN(200),    // base borrow rate
          new BN(400),    // slope1
          new BN(3000),   // slope2
          new BN(100_000_000) // mock price $100
        )
        .accountsPartial({
          market: marketPda,
          reserve: collateralReservePda,
          tokenMint: collateralMint,
          liquidityVault: collateralVaultPda,
          authority: authority.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([authority])
        .rpc();

      const reserve = await program.account.reserve.fetch(collateralReservePda);
      console.log("Collateral Reserve:", reserve);

      anchor.assert.strictEqual(reserve.isActive, true);
      anchor.assert.strictEqual(reserve.ltvBps.toNumber(), 7500);
      anchor.assert.strictEqual(reserve.mockPrice.toNumber(), 100_000_000);
    });

    it("initializes borrow reserve (price = $1 USDC)", async () => {
      await program.methods
        .initializeReserve(
          new BN(8500),   // ltv 85%
          new BN(9000),   // liquidation threshold 90%
          new BN(300),    // liquidation bonus 3%
          new BN(9000),   // optimal utilization 90%
          new BN(100),    // base borrow rate
          new BN(300),    // slope1
          new BN(2000),   // slope2
          new BN(1_000_000) // mock price $1
        )
        .accountsPartial({
          market: marketPda,
          reserve: borrowReservePda,
          tokenMint: borrowMint,
          liquidityVault: borrowVaultPda,
          authority: authority.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([authority])
        .rpc();

      const reserve = await program.account.reserve.fetch(borrowReservePda);
      console.log("Borrow Reserve:", reserve);

      anchor.assert.strictEqual(reserve.isActive, true);
      anchor.assert.strictEqual(reserve.mockPrice.toNumber(), 1_000_000);

      // Market should have 2 reserves now
      const market = await program.account.market.fetch(marketPda);
      anchor.assert.strictEqual(market.reserveCount.toNumber(), 2);
    });
  });

  // ─────────────────────────────────────────────
  // DEPOSIT
  // ─────────────────────────────────────────────
  describe("deposit", () => {
    it("user deposits collateral tokens", async () => {
      const depositAmount = new BN(100_000_000); // 100 tokens

      await program.methods
        .deposit(depositAmount)
        .accountsPartial({
          market: marketPda,
          reserve: collateralReservePda,
          userPosition: userCollateralPositionPda,
          liquidityVault: collateralVaultPda,
          userTokenAccount: userCollateralAccount,
          tokenMint: collateralMint,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      const position = await program.account.userPosition.fetch(userCollateralPositionPda);
      console.log("User Collateral Position:", position);

      anchor.assert.strictEqual(
        position.depositedAmount.toNumber(),
        depositAmount.toNumber()
      );

      const reserve = await program.account.reserve.fetch(collateralReservePda);
      anchor.assert.strictEqual(
        reserve.totalDeposits.toNumber(),
        depositAmount.toNumber()
      );
    });

    it("user deposits borrow tokens as liquidity", async () => {
      const depositAmount = new BN(500_000_000); // 500 USDC

      await program.methods
        .deposit(depositAmount)
        .accountsPartial({
          market: marketPda,
          reserve: borrowReservePda,
          userPosition: userBorrowPositionPda,
          liquidityVault: borrowVaultPda,
          userTokenAccount: userBorrowAccount,
          tokenMint: borrowMint,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      const reserve = await program.account.reserve.fetch(borrowReservePda);
      anchor.assert.strictEqual(reserve.totalDeposits.toNumber(), 500_000_000);
    });

    it("fails to deposit zero amount", async () => {
      try {
        await program.methods
          .deposit(new BN(0))
          .accountsPartial({
            market: marketPda,
            reserve: collateralReservePda,
            userPosition: userCollateralPositionPda,
            liquidityVault: collateralVaultPda,
            userTokenAccount: userCollateralAccount,
            tokenMint: collateralMint,
            user: user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user])
          .rpc();
        anchor.assert.fail("Should have thrown ZeroAmount error");
      } catch (err: any) {
        anchor.assert.include(err.toString(), "ZeroAmount");
      }
    });
  });

  // ─────────────────────────────────────────────
  // BORROW
  // ─────────────────────────────────────────────
  describe("borrow", () => {
    it("user borrows against collateral", async () => {
      // Collateral: 100 tokens @ $100 = $10,000
      // LTV: 75% => max borrow = $7,500 worth of USDC
      const borrowAmount = new BN(50_000_000); // 50 USDC

      await program.methods
        .borrow(borrowAmount)
        .accountsPartial({
          market: marketPda,
          reserve: borrowReservePda,
          collateralReserve: collateralReservePda,
          collateralPosition: userCollateralPositionPda,
          borrowPosition: userBorrowPositionPda,
          liquidityVault: borrowVaultPda,
          userTokenAccount: userBorrowAccount,
          tokenMint: borrowMint,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      const borrowPosition = await program.account.userPosition.fetch(userBorrowPositionPda);
      console.log("Borrow Position:", borrowPosition);

      anchor.assert.strictEqual(
        borrowPosition.borrowedAmount.toNumber(),
        borrowAmount.toNumber()
      );

      const reserve = await program.account.reserve.fetch(borrowReservePda);
      anchor.assert.strictEqual(reserve.totalBorrows.toNumber(), borrowAmount.toNumber());
    });

    it("fails to borrow more than LTV allows", async () => {
      // Trying to borrow $8,000 when max is $7,500
      const overLimitAmount = new BN(8_000_000_000);

      try {
        await program.methods
          .borrow(overLimitAmount)
          .accountsPartial({
            market: marketPda,
            reserve: borrowReservePda,
            collateralReserve: collateralReservePda,
            collateralPosition: userCollateralPositionPda,
            borrowPosition: userBorrowPositionPda,
            liquidityVault: borrowVaultPda,
            userTokenAccount: userBorrowAccount,
            tokenMint: borrowMint,
            user: user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user])
          .rpc();
        anchor.assert.fail("Should have thrown InsufficientCollateral error");
      } catch (err: any) {
        anchor.assert.include(err.toString(), "InsufficientCollateral");
      }
    });
  });

  // ─────────────────────────────────────────────
  // REPAY
  // ─────────────────────────────────────────────
  describe("repay", () => {
    it("user repays borrowed amount", async () => {
      const repayAmount = new BN(25_000_000); // repay 25 USDC

      const borrowPositionBefore = await program.account.userPosition.fetch(userBorrowPositionPda);
      const borrowedBefore = borrowPositionBefore.borrowedAmount.toNumber();

      await program.methods
        .repay(repayAmount)
        .accountsPartial({
          market: marketPda,
          reserve: borrowReservePda,
          userPosition: userBorrowPositionPda,
          liquidityVault: borrowVaultPda,
          userTokenAccount: userBorrowAccount,
          tokenMint: borrowMint,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      const borrowPositionAfter = await program.account.userPosition.fetch(userBorrowPositionPda);
      console.log("Borrow Position After Repay:", borrowPositionAfter);

      anchor.assert.strictEqual(
        borrowPositionAfter.borrowedAmount.toNumber(),
        borrowedBefore - repayAmount.toNumber()
      );
    });

    it("user fully repays remaining borrow", async () => {
      const repayAmount = new BN(1_000_000_000); // repay all (more than owed, capped internally)

      await program.methods
        .repay(repayAmount)
        .accountsPartial({
          market: marketPda,
          reserve: borrowReservePda,
          userPosition: userBorrowPositionPda,
          liquidityVault: borrowVaultPda,
          userTokenAccount: userBorrowAccount,
          tokenMint: borrowMint,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      const borrowPosition = await program.account.userPosition.fetch(userBorrowPositionPda);
      anchor.assert.strictEqual(borrowPosition.borrowedAmount.toNumber(), 0);
    });
  });

  // ─────────────────────────────────────────────
  // WITHDRAW
  // ─────────────────────────────────────────────
  describe("withdraw", () => {
    it("user withdraws deposited collateral", async () => {
      const withdrawAmount = new BN(10_000_000); // 10 tokens

      const positionBefore = await program.account.userPosition.fetch(userCollateralPositionPda);
      const depositedBefore = positionBefore.depositedAmount.toNumber();

      await program.methods
        .withdraw(withdrawAmount)
        .accountsPartial({
          market: marketPda,
          reserve: collateralReservePda,
          userPosition: userCollateralPositionPda,
          liquidityVault: collateralVaultPda,
          userTokenAccount: userCollateralAccount,
          tokenMint: collateralMint,
          owner: user.publicKey,
          user: user.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      const positionAfter = await program.account.userPosition.fetch(userCollateralPositionPda);
      console.log("Position After Withdraw:", positionAfter);

      anchor.assert.strictEqual(
        positionAfter.depositedAmount.toNumber(),
        depositedBefore - withdrawAmount.toNumber()
      );
    });

    it("fails to withdraw more than deposited", async () => {
      const overAmount = new BN(999_000_000_000);

      try {
        await program.methods
          .withdraw(overAmount)
          .accountsPartial({
            market: marketPda,
            reserve: collateralReservePda,
            userPosition: userCollateralPositionPda,
            liquidityVault: collateralVaultPda,
            userTokenAccount: userCollateralAccount,
            tokenMint: collateralMint,
            owner: user.publicKey,
            user: user.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([user])
          .rpc();
        anchor.assert.fail("Should have thrown InsufficientDeposit error");
      } catch (err: any) {
        anchor.assert.include(err.toString(), "InsufficientDeposit");
      }
    });
  });

  // ─────────────────────────────────────────────
  // LIQUIDATION
  // ─────────────────────────────────────────────
  describe("liquidate", () => {
    let undercollateralizedUser: Keypair;
    let undercollateralizedCollateralAccount: PublicKey;
    let undercollateralizedBorrowAccount: PublicKey;
    let undercollateralizedCollateralPosition: PublicKey;
    let undercollateralizedBorrowPosition: PublicKey;

    before(async () => {
      undercollateralizedUser = Keypair.generate();
      await provider.context.banksClient.requestAirdrop(
        undercollateralizedUser.publicKey,
        10_000_000_000
      );

      undercollateralizedCollateralAccount = await createAccount(
        provider.connection,
        undercollateralizedUser,
        collateralMint,
        undercollateralizedUser.publicKey
      );

      undercollateralizedBorrowAccount = await createAccount(
        provider.connection,
        undercollateralizedUser,
        borrowMint,
        undercollateralizedUser.publicKey
      );

      await mintTo(
        provider.connection,
        authority,
        collateralMint,
        undercollateralizedCollateralAccount,
        authority,
        10_000_000 // only 10 tokens
      );

      await mintTo(
        provider.connection,
        authority,
        borrowMint,
        undercollateralizedBorrowAccount,
        authority,
        100_000_000
      );

      [undercollateralizedCollateralPosition] = PublicKey.findProgramAddressSync(
        [Buffer.from("position"), collateralReservePda.toBuffer(), undercollateralizedUser.publicKey.toBuffer()],
        PROGRAM_ID
      );

      [undercollateralizedBorrowPosition] = PublicKey.findProgramAddressSync(
        [Buffer.from("position"), borrowReservePda.toBuffer(), undercollateralizedUser.publicKey.toBuffer()],
        PROGRAM_ID
      );

      // Deposit small collateral
      await program.methods
        .deposit(new BN(10_000_000)) // 10 tokens @ $100 = $1,000 collateral
        .accountsPartial({
          market: marketPda,
          reserve: collateralReservePda,
          userPosition: undercollateralizedCollateralPosition,
          liquidityVault: collateralVaultPda,
          userTokenAccount: undercollateralizedCollateralAccount,
          tokenMint: collateralMint,
          user: undercollateralizedUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([undercollateralizedUser])
        .rpc();

      // Borrow close to max (74% of $1,000 = $740 USDC)
      await program.methods
        .borrow(new BN(740_000_000))
        .accountsPartial({
          market: marketPda,
          reserve: borrowReservePda,
          collateralReserve: collateralReservePda,
          collateralPosition: undercollateralizedCollateralPosition,
          borrowPosition: undercollateralizedBorrowPosition,
          liquidityVault: borrowVaultPda,
          userTokenAccount: undercollateralizedBorrowAccount,
          tokenMint: borrowMint,
          user: undercollateralizedUser.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([undercollateralizedUser])
        .rpc();
    });

    it("fails to liquidate a healthy position", async () => {
      // Try to liquidate the main user who has no borrows
      try {
        await program.methods
          .liquidate(new BN(1_000_000))
          .accountsPartial({
            market: marketPda,
            borrowReserve: borrowReservePda,
            collateralReserve: collateralReservePda,
            borrowPosition: userBorrowPositionPda,
            collateralPosition: userCollateralPositionPda,
            borrowVault: borrowVaultPda,
            collateralVault: collateralVaultPda,
            liquidatorRepayAccount: liquidatorBorrowAccount,
            liquidatorCollateralAccount: liquidatorCollateralAccount,
            borrower: user.publicKey,
            borrowMint: borrowMint,
            collateralMint: collateralMint,
            liquidator: liquidator.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .signers([liquidator])
          .rpc();
        anchor.assert.fail("Should have thrown PositionHealthy error");
      } catch (err: any) {
        anchor.assert.include(err.toString(), "PositionHealthy");
      }
    });

    it("prints reserve utilization and borrow rates", async () => {
      const reserve = await program.account.reserve.fetch(borrowReservePda);
      console.log("\n📊 Reserve Stats:");
      console.log("  Total Deposits:", reserve.totalDeposits.toNumber() / 1_000_000, "USDC");
      console.log("  Total Borrows:", reserve.totalBorrows.toNumber() / 1_000_000, "USDC");
      const utilization = reserve.totalDeposits.toNumber() > 0
        ? (reserve.totalBorrows.toNumber() * 100) / reserve.totalDeposits.toNumber()
        : 0;
      console.log("  Utilization:", utilization.toFixed(2) + "%");
    });
  });
});
