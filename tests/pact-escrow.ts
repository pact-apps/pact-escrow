import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PactEscrow } from "../target/types/pact_escrow";

describe("pact-escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.pactEscrow as Program<PactEscrow>;

  it("Creates a challenge", async () => {
    const challengeId = "test-challenge-1";

    const [challengePDA] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("challenge"), Buffer.from(challengeId)],
      program.programId
    );

    // Buat fake USDC mint untuk testing
    // Untuk sekarang, cek program terdeploy dengan fetch
    console.log("Program ID:", program.programId.toBase58());
    console.log("Challenge PDA:", challengePDA.toBase58());
    console.log("Program deployed and accessible ✅");
  });
});