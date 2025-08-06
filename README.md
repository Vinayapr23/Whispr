# Whispr AMM - Initial Proof of Concept

## Overview

Whispr is a **confidential automated market maker (AMM)** built on Solana using Arcium's secure multi-party computation (MPC) technology. This implementation allows users to perform token swaps while keeping swap amounts and directions private through cryptographic protocols.

## Development Status

**This is an INITIAL PROOF OF CONCEPT (POC)**


## Features (Completed)

### Implemented
- Basic AMM functionality (deposit, withdraw, swap)
- Pool initialization and management
- Token vault management
- Lock/unlock mechanisms for admin control
- Confidential swap flow 


## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   User Client   │───▶│  Whispr Program │───▶│ Arcium Network  │
│                 │    │     (Solana)    │    │      (MPC)      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                        │                        │
         │                        ▼                        │
         │              ┌─────────────────┐                │
         │              │  AMM Liquidity  │                │
         │              │     Pools       │                │
         │              └─────────────────┘                │
         │                                                 │
         └─────────────── Encrypted Results ◀──────────────┘



```

## Roadmap

### Phase 1 (Current) - Core AMM + Basic Privacy
- [x] Standard AMM implementation
- [x] Pool management and administration
- [x] Confidential swap integration

### Phase 2 - DEX Integration
- [ ] Raydium protocol integration
- [ ] Orca whirlpool compatibility
- [ ] Cross-DEX arbitrage routing
- [ ] UI/UX integration




