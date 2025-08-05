


import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { Whispr } from "../target/types/whispr";
import { randomBytes } from "crypto";
import {
  awaitComputationFinalization,
  getArciumEnv,
  getCompDefAccOffset,
  getArciumAccountBaseSeed,
  getArciumProgAddress,
  uploadCircuit,
  buildFinalizeCompDefTx,
  RescueCipher,
  deserializeLE,
  getMXEAccAddress,
  getMempoolAccAddress,
  getCompDefAccAddress,
  getExecutingPoolAccAddress,
  x25519,
  getComputationAccAddress,
  getMXEPublicKey,
} from "@arcium-hq/client";
import * as fs from "fs";
import * as os from "os";
import { 
  ASSOCIATED_TOKEN_PROGRAM_ID as associatedTokenProgram, 
  TOKEN_PROGRAM_ID as tokenProgram, 
  createMint, 
  mintTo, 
  getAssociatedTokenAddress, 
  getOrCreateAssociatedTokenAccount,
  getAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import { SystemProgram, Keypair } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";
import { associatedAddress } from "@coral-xyz/anchor/dist/cjs/utils/token";

describe("Whispr", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program = anchor.workspace.Whispr as Program<Whispr>;
  const provider = anchor.getProvider();

  type Event = anchor.IdlEvents<(typeof program)["idl"]>;
  const awaitEvent = async <E extends keyof Event>(eventName: E) => {
    let listenerId: number;
    const event = await new Promise<Event[E]>((res) => {
      listenerId = program.addEventListener(eventName, (event) => {
        res(event);
      });
    });
    await program.removeEventListener(listenerId);

    return event;
  };

  const arciumEnv = getArciumEnv();

  // AMM variables (setup before the test)
  const [admin, user] = [new Keypair(), new Keypair()];
  const seed = new BN(randomBytes(8));
  const fee = 300;
  const DECIMALS = 6;
  const config = PublicKey.findProgramAddressSync(
    [Buffer.from("config"), seed.toArrayLike(Buffer, "le", 8)], 
    program.programId
  )[0];
  let mint_x: PublicKey;
  let mint_y: PublicKey;
  let mint_lp = PublicKey.findProgramAddressSync(
    [Buffer.from("lp"), config.toBuffer()],
    program.programId
  )[0];
  let vault_x: PublicKey;
  let vault_y: PublicKey;
  let user_x: PublicKey;
  let user_y: PublicKey;
  let user_lp: PublicKey;

  before("Setup AMM", async () => {
    // Airdrop
    await Promise.all([admin, user].map(async (k) => {
      const sig = await provider.connection.requestAirdrop(k.publicKey, 100 * anchor.web3.LAMPORTS_PER_SOL);
      const latestBlockhash = await provider.connection.getLatestBlockhash();
      await provider.connection.confirmTransaction({
        signature: sig,
        ...latestBlockhash,
      });
    }));
  
    // Create mints
    mint_x = await createMint(provider.connection, admin, admin.publicKey, admin.publicKey, DECIMALS);
    const info = await provider.connection.getAccountInfo(mint_x);
console.log("mint_x owner:", info?.owner.toBase58());
    mint_y = await createMint(provider.connection, admin, admin.publicKey, admin.publicKey, DECIMALS);

    // Get vault addresses
    vault_x = await getAssociatedTokenAddress(mint_x, config, true);
    vault_y = await getAssociatedTokenAddress(mint_y, config, true);

    // Create user accounts and mint tokens
    user_x = (await getOrCreateAssociatedTokenAccount(provider.connection, user, mint_x, user.publicKey, true)).address;
    user_y = (await getOrCreateAssociatedTokenAccount(provider.connection, user, mint_y, user.publicKey, true)).address;

    await mintTo(provider.connection, admin, mint_x, user_x, admin.publicKey, 1000 * Math.pow(10, DECIMALS));
    await mintTo(provider.connection, admin, mint_y, user_y, admin.publicKey, 1000 * Math.pow(10, DECIMALS));
   
    
    // Initialize AMM
    await program.methods
      .initializeAmm(seed, fee, admin.publicKey)
      .accountsStrict({
        admin: admin.publicKey,
        mintX: mint_x,
        mintY: mint_y,
        mintLp: mint_lp,
        vaultX: vault_x,
        vaultY: vault_y,
        config: config,
        tokenProgram,
        associatedTokenProgram,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    // Deposit liquidity
    user_lp = (await getOrCreateAssociatedTokenAccount(provider.connection, user, mint_lp, user.publicKey, true)).address;

    await program.methods
      .deposit(new BN(1000 * Math.pow(10, DECIMALS)), new BN(200 * Math.pow(10, DECIMALS)), new BN(200 * Math.pow(10, DECIMALS)))
      .accountsStrict({
        user: user.publicKey,
        mintX: mint_x,
        mintY: mint_y,
        config: config,
        mintLp: mint_lp,
        vaultX: vault_x,
        vaultY: vault_y,
        userX: user_x,
        userY: user_y,
        userLp: user_lp,
        tokenProgram,
        associatedTokenProgram,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

       const vaultXAcc = await getAccount(provider.connection, vault_x);
  const vaultYAcc = await getAccount(provider.connection, vault_y);
  const userXAcc  = await getAccount(provider.connection, user_x);
  const userYAcc  = await getAccount(provider.connection, user_y);
  const userLpAcc = await getAccount(provider.connection, user_lp);

  console.log("=== AMM Token Balances ===");
  console.log("Vault X:", Number(vaultXAcc.amount)  / 10**DECIMALS);
  console.log("Vault Y:", Number(vaultYAcc.amount)  / 10**DECIMALS);
  console.log("User  X:", Number(userXAcc.amount)   / 10**DECIMALS);
  console.log("User  Y:", Number(userYAcc.amount)   / 10**DECIMALS);
  console.log("User LP:", Number(userLpAcc.amount)  / 10**DECIMALS);
  });


  it("execute confidential swap", async () => {
    const owner = readKpJson(`${os.homedir()}/.config/solana/id.json`);

    const mxePublicKey = await getMXEPublicKeyWithRetry(
      provider as anchor.AnchorProvider,
      program.programId
    );

    console.log("MXE x25519 pubkey is", mxePublicKey);

    console.log("Initializing compute swap computation definition");
    const initSwapSig = await initComputeSwapCompDef(program, owner, false);
    console.log(
      "Compute swap computation definition initialized with signature",
      initSwapSig
    );

    const privateKey = x25519.utils.randomPrivateKey();
    const publicKey = x25519.getPublicKey(privateKey);
    const sharedSecret = x25519.getSharedSecret(privateKey, mxePublicKey);
    const cipher = new RescueCipher(sharedSecret);

    const isX = BigInt(1); // 1 = X->Y, 0 = Y->X
    const swapAmount = BigInt(10 * Math.pow(10, DECIMALS));
    const minOutput = BigInt(8 * Math.pow(10, DECIMALS));

    const nonce = randomBytes(16);
    const ciphertextIsX = cipher.encrypt([isX], nonce);
    const ciphertextAmount = cipher.encrypt([swapAmount], nonce);
    const ciphertextMinOutput = cipher.encrypt([minOutput], nonce);

    const swapExecutedEventPromise = awaitEvent("confidentialSwapExecutedEvent");

    const computationOffset = new anchor.BN(randomBytes(8), "hex");

    const swapStatePda = PublicKey.findProgramAddressSync(
      [
        Buffer.from("swap_state"),
       // user.publicKey.toBuffer()
      ],
      program.programId
    )[0];


  console.log(tokenProgram)
  console.log(associatedTokenProgram)

    const queueSig = await program.methods
      .computeSwap(
        computationOffset,
         Array.from(publicKey),
        new anchor.BN(deserializeLE(nonce).toString()),
      //  Array.from(ciphertextIsX[0]),
        Array.from(ciphertextAmount[0]),
      //  Array.from(ciphertextMinOutput[0]),

      )
      .accountsPartial({
        user: user.publicKey,
        mintX: mint_x,
        mintY: mint_y,
        config: config,
        mintLp: mint_lp,
        vaultX: vault_x,
        vaultY: vault_y,
        swapState: swapStatePda,
        userX: user_x,
        userY: user_y,
        computationAccount: getComputationAccAddress(
          program.programId,
          computationOffset
        ),
        clusterAccount: arciumEnv.arciumClusterPubkey,
        mxeAccount: getMXEAccAddress(program.programId),
        mempoolAccount: getMempoolAccAddress(program.programId),
        executingPool: getExecutingPoolAccAddress(program.programId),
        compDefAccount: getCompDefAccAddress(
          program.programId,
          Buffer.from(getCompDefAccOffset("compute_swap")).readUInt32LE()
        ),  
        tokenProgram,
        associatedTokenProgram,
        systemProgram: SystemProgram.programId,
      
      })
      .signers([user])
      .rpc({ commitment: "confirmed" });
    console.log("Queue sig is ", queueSig);
    
    const finalizeSig = await awaitComputationFinalization(
      provider as anchor.AnchorProvider,
      computationOffset,
      program.programId,
      "confirmed"
    );
    console.log("Finalize sig is ", finalizeSig);

    const swapExecutedEvent = await swapExecutedEventPromise;

    console.log(swapExecutedEvent);
  });















  async function initComputeSwapCompDef(
    program: Program<Whispr>,
    owner: anchor.web3.Keypair,
    uploadRawCircuit: boolean
  ): Promise<string> {
    const baseSeedCompDefAcc = getArciumAccountBaseSeed(
      "ComputationDefinitionAccount"
    );
    const offset = getCompDefAccOffset("compute_swap");

    const compDefPDA = PublicKey.findProgramAddressSync(
      [baseSeedCompDefAcc, program.programId.toBuffer(), offset],
      getArciumProgAddress()
    )[0];

    console.log("Comp def pda is ", compDefPDA.toBase58());

    const sig = await program.methods
      .initComputeSwapCompDef()
      .accounts({
        compDefAccount: compDefPDA,
        payer: owner.publicKey,
        mxeAccount: getMXEAccAddress(program.programId),
      })
      .signers([owner])
      .rpc({
        commitment: "confirmed",
      });
    console.log("Init compute swap computation definition transaction", sig);

    if (uploadRawCircuit) {
      const rawCircuit = fs.readFileSync("build/compute_swap.arcis");

      await uploadCircuit(
        provider as anchor.AnchorProvider,
        "compute_swap",
        program.programId,
        rawCircuit,
        true
      );
    } else {
      const finalizeTx = await buildFinalizeCompDefTx(
        provider as anchor.AnchorProvider,
        Buffer.from(offset).readUInt32LE(),
        program.programId
      );

      const latestBlockhash = await provider.connection.getLatestBlockhash();
      finalizeTx.recentBlockhash = latestBlockhash.blockhash;
      finalizeTx.lastValidBlockHeight = latestBlockhash.lastValidBlockHeight;

      finalizeTx.sign(owner);

      await provider.sendAndConfirm(finalizeTx);
    }
    return sig;
  }
});

async function getMXEPublicKeyWithRetry(
  provider: anchor.AnchorProvider,
  programId: PublicKey,
  maxRetries: number = 10,
  retryDelayMs: number = 500
): Promise<Uint8Array> {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      const mxePublicKey = await getMXEPublicKey(provider, programId);
      if (mxePublicKey) {
        return mxePublicKey;
      }
    } catch (error) {
      console.log(`Attempt ${attempt} failed to fetch MXE public key:`, error);
    }

    if (attempt < maxRetries) {
      console.log(
        `Retrying in ${retryDelayMs}ms... (attempt ${attempt}/${maxRetries})`
      );
      await new Promise((resolve) => setTimeout(resolve, retryDelayMs));
    }
  }

  throw new Error(
    `Failed to fetch MXE public key after ${maxRetries} attempts`
  );
}

function readKpJson(path: string): anchor.web3.Keypair {
  const file = fs.readFileSync(path);
  return anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(file.toString()))
  );
}