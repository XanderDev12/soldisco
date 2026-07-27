export interface RpcContext {
  slot: number;
  fetchedAt: string;
  endpointId: string;
}

export interface MintSnapshot {
  mint: string;
  decimals: number;
  supply: string;
  mintAuthority: string | null;
  freezeAuthority: string | null;
  executable: boolean;
  context: RpcContext;
}

export interface TokenAccountConcentration {
  mint: string;
  largestAccounts: Array<{
    address: string;
    rawAmount: string;
    shareOfSupply: number | null;
  }>;
  context: RpcContext;
}

export interface SignatureActivityWindow {
  address: string;
  fromSlot: number;
  toSlot: number;
  signatureCount: number;
  context: RpcContext;
}

export interface SolanaRpcReader {
  getMintSnapshot(mint: string): Promise<MintSnapshot>;
  getLargestTokenAccounts(mint: string, limit: number): Promise<TokenAccountConcentration>;
  getSignatureActivity(address: string, fromSlot: number): Promise<SignatureActivityWindow>;
}

export interface SolanaRpcConfig {
  endpointId: string;
  endpointUrlRef: string;
  commitment: "processed" | "confirmed" | "finalized";
  timeoutMs: number;
}
