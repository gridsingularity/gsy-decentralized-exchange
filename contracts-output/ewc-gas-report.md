# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-28T15:11:23.430Z
- **network**: ewc (246)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: EWT
- **settleBatchSizes**: 1,2,5
- **matchTypes**: Standard=0,Preferred=1
- **actorRegistryProxy**: 0x00F20873913fbf9b88ADBE024e6d5b4a9EcBf83b
- **marketControllerProxy**: 0x4e8F7F7B7e9dC3b8141987a07671780fD5079fc1
- **orderRegistryProxy**: 0x751A1bDB22b8B4D0b6A96228D6C4B42D15872A02
- **tradeSettlementProxy**: 0x04F26873a0Da1bc85ff162b7d94d54725583d7cf

## Totals

- Deployment gas: 7900661
- Deployment fee: 0.00086907271 EWT
- Role setup gas: 374145
- Role setup fee: 0.00004002346 EWT
- Mutating call gas: 23618617
- Mutating call fee: 0.00300212767097874 EWT
- Deployment + setup + mutating fee: 0.00391122384097874 EWT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (EWT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 110000000 | 0.00006944201 | `0xbfa15dcbc04475dd7b6be11f5b2dfb1f98f892e72acf312dc5793e852caa43f2` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720527 | 110000000 | 0.00007925797 | `0x7177a5b7c9e13a3387d52334fc7e22f4e10a75732a3c7ac90d0584880ba02137` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 885027 | 110000000 | 0.00009735297 | `0x0473b89496ccfc475719be9bbcd5e86d6c14989f2bd08eb18f9b08df1528e89e` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695826 | 110000000 | 0.00007654086 | `0x04a444badf78aad5dc5896fe3b290414a740f7f32582486affd3db6780abba1a` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1498037 | 110000000 | 0.00016478407 | `0x9ae37b03b9cf2edee665d46e082d1ba3fb52ddf033bfbc4f308669e85321e8e2` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 718504 | 110000000 | 0.00007903544 | `0x5272eb31a8d4ab4c1dcbd476cfe430d6d7396495d59a5e99cf7912154404c118` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 2032901 | 110000000 | 0.00022361911 | `0x90384e56971faa874f5f8db2bff55977b971df93817899bf001958b7fd9bf5f0` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718548 | 110000000 | 0.00007904028 | `0x226a7b653c322a69d2a75798828ad3acea2ef0e5ce6b66c2102c5defcdc0c956` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 110000000 | 0.00000622985 | `0x19f65aef8cf758956e42ddfbd3c4d7e285459f89276d47ff650a201f183651c9` |  |
| Role setup | OrderRegistry | grantRole(COMMUNITY_CLIENT_ROLE) | 56658 | 110000000 | 0.00000623238 | `0x0891da158bcfdfd50c9dc966e0ff511e76f60ed401a851dbb24ca805d21f07ad` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56658 | 110000000 | 0.00000623238 | `0xbc10333850117233b47e37022c027dff5500296a05003b196c9e0874012ed584` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56658 | 100000000 | 0.0000056658 | `0x55c7893f0073b9c4bf53cf9ab3138065ac5391856cb72ac5e891e16f7a65913b` | Benchmark-only grant used to measure updateStatus and settleOrder directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56591 | 100000000 | 0.0000056591 | `0xf5fa1d5d4ee3c9ac933024308d35fc6479940eb8a8326d30fe0aab14e006c124` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56591 | 110000000 | 0.00000622501 | `0x044ef3caef43e04c92ee90d579a7e0f5954bfc6790320d4a4026d7905113ef7f` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 110000000 | 0.00000377894 | `0x6f582f122e776204ee7ef5d3ed8e0494b7137995dc1dccbfaf4809bd6513275c` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 110000000 | 0.00000592658 | `0xf7decf9b6212bc17e87ffe90595d0afa19fba1454ce1ffc2d61f620d194a3c8d` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 110000000 | 0.00000592658 | `0x97314cde7eb206cc57ace336f3f70be7df40fe95cdcfd003a18150fca2977da4` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 3000000007 | 0.000162243000378567 | `0x755c3b5ebe24054ff2e200bddc4a8d15a92058181be3453fd9bf7af8d1466d13` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 3000000007 | 0.000096507000225183 | `0xc123ff5ab72d995989605323ae5cb927c079868e7e1e6f4b67fe0130ef2c02d7` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 3000000007 | 0.00016071000037499 | `0x51eacab69798696462159cbba6d569713bc98265cdafce716bbadd2f794fca7e` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 110000000 | 0.00000348238 | `0x23a5e83c5177c83308c7e50a73a27e785546daad1f579d60c97634a857666652` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | createMarkets(1 market) | 83354 | 110000000 | 0.00000916894 | `0x025bdaed938c49c357cabf2bbac4a25e93d4722903ab4b3f65b0adbd666c67d2` |  |
| Mutating calls | MarketController | createMarkets(3 markets) | 191471 | 110000000 | 0.00002106181 | `0x6dcaa1b261cac20ac6aef0a623b4a31af9bb576279fb35b8a735082e16d37345` | One community's spot, flex and settlement markets for one delivery slot. |
| Mutating calls | MarketController | createMarkets(3 markets, all existing) | 41784 | 110000000 | 0.00000459624 | `0x59b5160d1ab5d106a73b924c0aca6623685d7bcd09261af9b009d28a6f5e3362` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | MarketController | createMarkets(50 markets) | 2732427 | 110000000 | 0.00030056697 | `0x75ad2841dc5276e0a216a57c97b1155644524900f7a4c10ca851272afcf74eb0` |  |
| Mutating calls | MarketController | createMarkets(50 markets, all existing) | 237724 | 110000000 | 0.00002614964 | `0x3f89900479fe6f9fe4d67be8e2323a7278b27eef83ce578edc69d388e134bb10` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 147551 | 110000000 | 0.00001623061 | `0xb6da6edefdaf424d693ee86ef4926ec46a04420f0653ee0c834915096e84563c` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 147539 | 110000000 | 0.00001622929 | `0xe615177231f8d7f6df9e26c2ce0ffc3bea91a2a05126f079e060751fac7f70f0` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 147551 | 110000000 | 0.00001623061 | `0xc0165f67dc43122d2aa75ee805f3e926141cd4cad221a4d705a061d748cb2323` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 57925 | 110000000 | 0.00000637175 | `0xdd4ff0a29755d705ca5e68e2e61c16d100cb4d8d98afddc97702952bcd81cfc2` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 147551 | 110000000 | 0.00001623061 | `0xe1a0c5df3e50fe3fc3faf6b5acf19349ca47e09b11969521103c4a3208b465e6` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 36083 | 110000000 | 0.00000396913 | `0xe57a637ebb8cd94a5a77c2938e8ab33104d4ec0abc64b14840e9911f89dbfc7d` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleOrder benchmark | 147551 | 110000000 | 0.00001623061 | `0xe1bb8deb278713c9f7bb83c3b630e328efec51c8709c96ef0cbd529ab03cbf99` |  |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) partial fill | 160541 | 110000000 | 0.00001765951 | `0x00f98a54fd365c19630c5c45817ef1493e76b1b9a1a2aefdf884b414a8cf0dc9` | Benchmark-only direct call; consumes parent and registers a 50000 residual bid with inherited metadata. |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) consume residual | 61310 | 110000000 | 0.0000067441 | `0x6262a3f03051dee40c92f0c5a143a2ba84d40252ab8ed7f91bc9d0f6e6a5388e` | Benchmark-only direct call; fully consumes the previously registered residual bid. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 147551 | 110000000 | 0.00001623061 | `0xf4d2a7635b405ce777847bd27052345fbaeaa41c715646a36ba36257b321928a` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 147539 | 110000000 | 0.00001622929 | `0xe753a5980bd7d10560cf4e456cd74a3c54a6212e51b7ed794467c5156b4ca44b` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 147539 | 110000000 | 0.00001622929 | `0xfb7afa2382b0eeb69fe9ee84117c60f9a09256f4a47bf571f7822bc325764f5a` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 147539 | 110000000 | 0.00001622929 | `0x3e94747096b6681efc811f7c951faa9e69b58740f99cbd0d750ee91f64d7bd2c` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 147551 | 110000000 | 0.00001623061 | `0x65c3a1b900cfd3bf7b07a56d31a02ef14817a482913043a459ff7f5eb1853ac7` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 147539 | 110000000 | 0.00001622929 | `0xdc794444fea60b54d1312553d5e45f1f4efcd7335930c227a2435fafc5c87954` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 147551 | 110000000 | 0.00001623061 | `0x3e74675128c1427c42b460e59b345aede9ff98462e3912c1a54eb273ebe3891e` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 147539 | 110000000 | 0.00001622929 | `0xfc95d9de05db5c13afcc594f27cc18661008a9ec26ea67ab77a5bcbde4f646bd` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 147551 | 110000000 | 0.00001623061 | `0x736a683deb423dabc989b0a485363aab40d26da729407bb3679b3fc54e1d9372` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 147527 | 110000000 | 0.00001622797 | `0x15052e5a55a6301d5980027023bc191b265eb2cdd8742d570d3647ca899d1b8f` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 147551 | 110000000 | 0.00001623061 | `0x1ca85a3089bfe0e5b23432c08b09006d1aa07b141c99bf74a2e321865579b29e` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 147539 | 110000000 | 0.00001622929 | `0xb50e2a8f87a05a0cebe61d3c61e98e1980e1ce4dc573d0d8a0703aca8865f073` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 147551 | 110000000 | 0.00001623061 | `0x67f334b99473f69b38865c595e678daca44c0ca541a5e68039e1a17aa479bbc6` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 147539 | 110000000 | 0.00001622929 | `0x4a0400202dbe5ff077dde22a125a0cca034b9a1305e54d49a0adde6f4880b214` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 266016 | 110000000 | 0.00002926176 | `0x3a7b0f951cc6b8dbf7dcb3c23c8aaf27587615885045247ae668688d63ab864a` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 473062 | 110000000 | 0.00005203682 | `0x57f30e6b3e6e7443ba28e8d7b1b4d6ae4b86070c1bb524f56134f43fbe679953` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 1094309 | 110000000 | 0.00012037399 | `0x9902e7aaf6e6c2e2fa7a175fefa91a1772bc1aee21cdeb481b7b5fa7a60600a1` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] bid 1 | 167655 | 110000000 | 0.00001844205 | `0x7c250c2b2897289af05b6f79b74e5bd0cf009d5d1ccae9c08107b54f9b755c05` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] offer 1 | 147539 | 110000000 | 0.00001622929 | `0xafc1d7d72258f173619fc4fec5807cf30e9d53c3a21fb7e80d0f8f1c38a78e00` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred buyer-only | 267499 | 110000000 | 0.00002942489 | `0x9e7e2fd8f9bc62eccdb76d2ccc4e2940555e98864c648dccf0121808cd3835f5` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 1 | 167667 | 110000000 | 0.00001844337 | `0x22a0c58e5a1d5f67ab3cf29078d04665db1e6a700f3d242f3b2620b8184ed7f1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 1 | 147527 | 110000000 | 0.00001622797 | `0x096445c77ecfae90bea70c8e4689dc13c85fc5e22f719fe41f64ec71d009ab90` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 2 | 167667 | 110000000 | 0.00001844337 | `0x29bec724f29231c5a5f25cec89c38ec1b24b8f6c2ffbdadd02bbfbcf162e5f90` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 2 | 147539 | 110000000 | 0.00001622929 | `0x3ddf15afb339b9c75d03cd7bfb1a5e2c3758a0f970d97c6f77aa89e230f46d35` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred buyer-only | 476052 | 110000000 | 0.00005236572 | `0x0c15ba66bb41bec7a767db62fc2e38d63db63e3a3143fbd3243c98012fb0a1df` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 1 | 167655 | 110000000 | 0.00001844205 | `0x56946108c4c670fd8ff49c61547e220835ffa3026969914e0d16c64408857631` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 1 | 147539 | 110000000 | 0.00001622929 | `0x65184c31c6a361cfe54233b0a28094155c30e6fc4f7fc04d2895afd81a39d3db` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 2 | 167655 | 110000000 | 0.00001844205 | `0x93df83fb277735237d8f8e8766a58743c484c74daf29229fe9d9642836aa90ed` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 2 | 147539 | 110000000 | 0.00001622929 | `0x5a398cc6eb89314fc6dcc887110a8ed2fd36afcced67d56fa32a00321846e33a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 3 | 167667 | 110000000 | 0.00001844337 | `0xb54872bf2c154e203390db1b109cef38401a76fcc4a922a2d5db463dd8afe84e` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 3 | 147539 | 110000000 | 0.00001622929 | `0x9c9276463c15356801c4148238a91d5981beaa19a522ac3ea72ac51a800bcc7a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 4 | 167667 | 110000000 | 0.00001844337 | `0x10bc0f713402f191c6d1d44dad82de23705338a7eea0578c3f905e50aaa0546b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 4 | 147539 | 110000000 | 0.00001622929 | `0x44f9f42db204f3ee2bfb36541ca3b569e780bb056d0db6ca7d9251636cf43383` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 5 | 167667 | 110000000 | 0.00001844337 | `0xf50aeb5eb4d34f2356d3f4854aea52e977d4bf1134cfd07d028a8c7835dbf7c1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 5 | 147539 | 110000000 | 0.00001622929 | `0x09871ca1b51ea2a29b46beb8abe0c5741998c1584c4785870828c90c88a5e35d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred buyer-only | 1101772 | 110000000 | 0.00012119492 | `0x10d398b6f2769e487c738c4d657af863b430c5805e1dce3a4a70bcb908aa577f` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] bid 1 | 147551 | 110000000 | 0.00001623061 | `0x8f97a0d87f385b09713a2ca9ce77b2563f52386e8b62f98603c5bfd1918dce75` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] offer 1 | 167655 | 110000000 | 0.00001844205 | `0xdb8de73a053acb4b133884ed925b03df1445596c1de15b9b4a27b0504c141e82` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred seller-only | 287440 | 110000000 | 0.0000316184 | `0x7e80254e7932079d3afa0992d71ef281ea930992123d1126d7929b43fa1fe4f9` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 1 | 147551 | 110000000 | 0.00001623061 | `0x63d561b4202744ecf575c78bd693b0636771dcda90552ab00cbab711a43499f5` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 1 | 167655 | 110000000 | 0.00001844205 | `0x1e0c11a82595e6e89cce9f983c2afa13fd9e2e3e582cebcb30131bb7158041b1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 2 | 147551 | 110000000 | 0.00001623061 | `0xb32f1b3de778c38aa89d3ba226c7ded7450ed8f0c056e2bd5b243de41cd5f0aa` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 2 | 167655 | 110000000 | 0.00001844205 | `0x79c6dcacc28b8e341aa3c0912ead9bdb28261dac912c148a986c13eb6e0d71f3` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred seller-only | 515922 | 110000000 | 0.00005675142 | `0xb44f596f437c1cbaa4f48b3cf129d89844401f17e2bace1a74d9c42e2d105e1a` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 1 | 147551 | 110000000 | 0.00001623061 | `0x51f14a4c5f95da119091bf2d57d9c44e3ac765032560e0b1b44c62c0985934aa` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 1 | 167655 | 110000000 | 0.00001844205 | `0x34dba633c69bda9a8fed59289539d47e13b880d669172e8a7ac901970983488d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 2 | 147551 | 110000000 | 0.00001623061 | `0xe4410d0b1affc15e0bc0a22bb22dd9094e4820bcee34827d55e796369b712d04` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 2 | 167643 | 110000000 | 0.00001844073 | `0xb803abe6126b7b40bf1753e2ff017486f601bba3b30c73815a97ecb522fe5200` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 3 | 147551 | 110000000 | 0.00001623061 | `0x656a00cf7eca8fb5a1de0bc66ae1b2ec08399bfb440803a4b46199fc7ba19b70` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 3 | 167655 | 110000000 | 0.00001844205 | `0xbeccd669c68c593b8641edf9346d7f03b6c02eed6818069907e7da3050a96d63` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 4 | 147551 | 110000000 | 0.00001623061 | `0x770515d52f0d8e09ea6f8c3e775bca1366bb4d99f9b480ca9591f55f9af36728` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 4 | 167655 | 110000000 | 0.00001844205 | `0xf55f87585767a779416b75ab7d5f3d15d6d251f5b7e0860462bd99161a68c94e` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 5 | 147551 | 110000000 | 0.00001623061 | `0x71c64ebea69c3fd8b8221b875b10717c53f1a91daf27bd1d27e8b341dfa2bfcc` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 5 | 167655 | 110000000 | 0.00001844205 | `0x8ff12910dd02268119601eff84bf2537271ae577db0764ca333f1b2cda2ce035` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred seller-only | 1201441 | 110000000 | 0.00013215851 | `0x8f4fa5fbca81f9cc8400c42693ea64cfe1ac285026b37ef88b7cb5d86dbccd07` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] bid 1 | 167667 | 110000000 | 0.00001844337 | `0x3e9b875c13e1f68109a2eacd26dc784f1c27cf579c559072c2089d1b93c4c5c2` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] offer 1 | 167655 | 110000000 | 0.00001844205 | `0xf7285e53d59170e95baafd870516cc2901246f4294e9f91f1f47638c3794a327` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred reciprocal | 287847 | 110000000 | 0.00003166317 | `0xb0ca74c11ac3d7393d643ec6ac616a8ea21d7a5ca92e18f86e5438b028d0202c` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 1 | 167667 | 110000000 | 0.00001844337 | `0xc33cb7981c49a43345262f705623eaa133992169f4013581a6d9c4402dae96ef` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 1 | 167655 | 110000000 | 0.00001844205 | `0x682ce1cd28a826f89d87486a75641ea1f5f62a3e35d1979798c6730f7e44914d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 2 | 167655 | 110000000 | 0.00001844205 | `0x439d30f3fc9cc30a8184b7bae1ed709124bdaff9d0e9a534e5e223d1668d5ac5` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 2 | 167655 | 110000000 | 0.00001844205 | `0xfabd6ad411ca183ca6550ed149d5c5abe146092c6e5a341d1f445fa3c8a54041` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred reciprocal | 516724 | 110000000 | 0.00005683964 | `0xc0fbbc07b503e1795151d710d5b785b50dc86e5dbde597b47792c01da3a5bc93` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 1 | 167667 | 110000000 | 0.00001844337 | `0x2b973fca2c427e1324700ff7609df29ee845cf7d5a774aa6cedf0c3f3896f33b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 1 | 167655 | 110000000 | 0.00001844205 | `0x990a2fdafde78d527941190ffdd72203e76587772c248b216f27264e7cb51589` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 2 | 167667 | 110000000 | 0.00001844337 | `0x3c1255a874642cb00db89e8424d4e4a6d3f3d67b065a17711f095b2911e3f069` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 2 | 167655 | 110000000 | 0.00001844205 | `0x4ea5147b24b30b97c36e07a0179fe808e5e19b74340a3c6d9d266fb5dc48a0e0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 3 | 167655 | 110000000 | 0.00001844205 | `0x5156e25e4ccdecaecd0fda5b64f47eeb8be81bf9f5e575dc890f41ffcf93c5ab` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 3 | 167655 | 110000000 | 0.00001844205 | `0x87f12d2330777e86e8f8569dff1a9b7b93bcae26a343936f7e05790736f10cf7` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 4 | 167667 | 110000000 | 0.00001844337 | `0x5b438ca89530cb1858e354d77b9c83540628932ed9a39c04c3a2fe60d5a77bb0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 4 | 167655 | 110000000 | 0.00001844205 | `0x51abe4166e9280e1375167be547a0c103f2e768b084ba221d4df992b2087af66` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 5 | 167655 | 110000000 | 0.00001844205 | `0x06045f57a31d69bdd0c5927c378271592bb8802927c1f43688d11abf0400ffdd` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 5 | 167655 | 110000000 | 0.00001844205 | `0x55cca954fb81da123ce41a2951485a9b10c6e31d46b295cd95425496e378d05c` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred reciprocal | 1203452 | 110000000 | 0.00013237972 | `0x128b76ca84c2a80f6d752cd3505cdd14fb45f043b9b14afd597999a8cff20bec` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 104493 | 110000000 | 0.00001149423 | `0xaf04c25e283aefb02c7ce16eff67a875c50f1bcee7dbbd063d5ce7ac9af7389d` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification zero penalties bid | 147551 | 110000000 | 0.00001623061 | `0x974d35981e6b209855d4b3ade4b805250704205354351ac37398d8ba004d861a` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification zero penalties offer | 147539 | 110000000 | 0.00001622929 | `0x5a217fee3eb6ae3aa8684bf0998c2a83813cb1b6f0d7351121b6c4e35c5927ca` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) verification zero penalties | 266016 | 110000000 | 0.00002926176 | `0x9813af487f1c0ee9eea54645386e562bc2ff38e250ca88ad28edb7edcc2987ba` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | completeDeliveryVerification(DeliveryVerification[1]) zero penalties | 120599 | 110000000 | 0.00001326589 | `0xe4dc686c1b525b563a5e4167f8963d593d988312935fe1b20c9d64b808c25c56` | Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification seller penalty bid | 147551 | 110000000 | 0.00001623061 | `0xc7855a5bdf365fa504dee2230bf9b18fb0aec54dd5a7e46a90cefe60cb15134f` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification seller penalty offer | 147539 | 110000000 | 0.00001622929 | `0xd34b73e4118b2e22b36d27e06d1e044e81d71295d561d26e737a3394cfb4be39` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) verification seller penalty | 266016 | 110000000 | 0.00002926176 | `0xb59d248d7c25bfc93d0a82912ba68e291171813964bd0d4f5045a5e846a0e944` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | completeDeliveryVerification(DeliveryVerification[1]) seller penalty | 168799 | 110000000 | 0.00001856789 | `0x563b9ec6d6c7212a21a7794b17249ad837ccf212ec109de647f46f12b4cfbdcd` | Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately. |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29278 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 31285 |  |  | view estimate |  |
| View estimates | MarketController | marketsExist(3 markets) | 36192 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29158 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 36868 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28893 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Standard (matchType=0) and preferred (matchType=1: buyer-only, seller-only, reciprocal) batches each fill 100000 of a 100000 bid and a 150000 offer per match, at price 12000. Each match includes on-chain creation of a 50000 residual offer.
- Preferred benchmarks set both effective rates to 12000. The counterpart without a preferred partner uses its normal rate when no preferred rate is supplied. Differences include calldata and storage access costs, not just validation instructions.
- Direct `settleOrder` rows use a benchmark-only settlement role. Production `settleBatch` rows already include these internal calls; do not add their gas again when estimating a trade.
- Totals include all benchmark fixtures and alternative cases, not the cost of a single production deployment/trading workflow. Local fees are illustrative, not remote-network price estimates.
- Mainnet/Volta values depend on live gas price at execution time.
