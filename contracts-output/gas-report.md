# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-20T07:15:33.414Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **matchTypes**: Standard=0,Preferred=1
- **actorRegistryProxy**: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
- **marketControllerProxy**: 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
- **orderRegistryProxy**: 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
- **tradeSettlementProxy**: 0xa513E6E4b8f2a923D98304ec87F64353C4D5C853

## Totals

- Deployment gas: 7460717
- Deployment fee: 0.006026893641898417 ETH
- Role setup gas: 339814
- Role setup fee: 0.000116703547974983 ETH
- Mutating call gas: 21595056
- Mutating call fee: 0.000316014675751523 ETH
- Deployment + setup + mutating fee: 0.006459611865624923 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 885275 | 981513999 | 0.000868909805464725 | `0x3331f00bc7ea090f677d8eac3e18433fe3d278de539e0beb39e1c36d70ba6806` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696148 | 866065664 | 0.000602909879862272 | `0x6edae9a47888ad068b50ec7bef0c9d89194f3d5998ecee5a9478ad53c6e39f3c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1450498 | 762831706 | 0.001106485863889588 | `0x3fdcbbef130cd0727799fd961812e31ac6d580e8022cac7f9fe2dd55acf03236` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741598 | 676698459 | 0.000501838223797482 | `0xdf50c57fd8aacb018f1613d50f4d466636a49e1b279b7e1c996600a5f4d88a98` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1616034 | 596293137 | 0.000963629983358658 | `0xe8dac175dcf32edc29295c7de722164d6cb50c927d45d379749fc70c97b6f4c8` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718850 | 529786745 | 0.00038083720164325 | `0xc922945753fc8de2731d3f6ab3b363f07f135e20529cf7a9d10a273a16c2bcde` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 466737047 | 0.000026433652656845 | `0x46ae25169cfb6d1648be64836df37dfa80be9c47dde3d2ee3c3147e818226d61` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56658 | 408615197 | 0.000023151319831626 | `0x17e7a23bf27fac1da3077f63d8b72e5e654a75519a82ab517088f23ab1a87925` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56658 | 357731226 | 0.000020268335802708 | `0xb165dacb876912380e497f5b7f8d983af31ed67c8c2de9049e621a34b1a5ff57` | Benchmark-only grant used to measure updateStatus and settleOrder directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 313183725 | 0.0000177374734491 | `0xf2eaf075d2c1f6abbee87941f8a38ac959a299e7180dd4603eabd36c3c68fe5c` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 274183572 | 0.000015528660783792 | `0x7e198f8757345feb500ca0fefd93479f8440df88d1af3c119b1a81b99e26c2bb` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 240040032 | 0.000013584105450912 | `0x7818fd09b02652283e329b9b36e4f0ac22f28d6bb1ec94e9a8c8dd91e273e431` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 210148230 | 0.00001132236633594 | `0x7b0f64ade16b69c1a98755adab3bbd3d9328be2380e329c11433bac72caa4643` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 183974054 | 0.000009912154081412 | `0x23366089cf1b0cebf6625240ef2a68a29487a34abdf8eb3c4613dc225a9db95f` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 161059900 | 0.0000087102804519 | `0xda3fa375270c99e664e7455b27fac4fd0c9ff3006dc503f3ff7fcad28615d9ba` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 140999999 | 0.000004535828967831 | `0xd82acf00015c5b4c6a6a4222df6d3ad5519383881a0a7ef8257f072666bf2a74` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 123412798 | 0.000006636399799652 | `0x9ebb9b725a863bc808624b9bd552a5ebe29e94fa0a283243aaceb98e0a8bcb6d` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 108041503 | 0.000003442418368586 | `0x65a1b71eef34f8e5e69182ab32a6beb0ead5346d39845f8ec8ac6a648acbf718` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | createMarkets(1 market) | 83354 | 94565002 | 0.000007882371176708 | `0xa64fab061c2dff50ce9cac595d9b1189d563124daa50972249e39b10de2390be` |  |
| Mutating calls | MarketController | createMarkets(3 markets) | 191471 | 82810065 | 0.000015855725955615 | `0xbbfcb6bc5cec3b9cc5e976fa151f9a3eeb7e11c52c1819fa0bdc365070e9b527` | One community's spot, flex and settlement markets for one delivery slot. |
| Mutating calls | MarketController | createMarkets(3 markets, all existing) | 41784 | 72590938 | 0.000003033139753392 | `0x39c71f95a61bc9b1930f4f3640885801a65c1effcf88ec066ab44d27ebfcddbe` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | MarketController | createMarkets(50 markets) | 2732427 | 63542347 | 0.000173624824586169 | `0xbbc4135e5824c678e39334449e1e07d5d7fc1f7979754dd57ada71f476e73b2c` |  |
| Mutating calls | MarketController | createMarkets(50 markets, all existing) | 237724 | 57046428 | 0.000013561305049872 | `0xf008c0301269ad3c242de5bf35e2741d8e8f12fb12752814fa7b15d3767bf4a3` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 158313 | 50028636 | 0.000007920183451068 | `0x4258f30975d48e76327849cf89746be499fe15b9364fcfbd0e5ac77e354e1cfa` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 158301 | 43841058 | 0.000006940083322458 | `0x4021d15c18aaefbed1e92981d9185b7f587381e8f7e359799876137e1b615bd8` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 158313 | 38418760 | 0.00000608218915188 | `0xe9224b0976d162b9a2503799544482349d1262e52f1e617e13958167a060efbc` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51378 | 33667100 | 0.0000017297482638 | `0x320d1ea01d1db0b43419b046d286e1cb73ce5f9f787b297dce079c8aae52480b` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 158313 | 29473128 | 0.000004665979313064 | `0x4405fedc0a2825b3f3f7b863dd3bce4b35ebfebf723ecace86f387fa62e96bd1` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35972 | 25827871 | 0.000000929080175612 | `0xbe214235b58e68ec0cb6899575afb66104bd41a9787ddc9dcf7eb2ab7e857365` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleOrder benchmark | 158313 | 22607130 | 0.00000357900257169 | `0xac559a867f07555e4bf4db7b1c6b9a9f323c5f00477814096197a7f842e8ad05` |  |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) partial fill | 145074 | 19811064 | 0.000002874070298736 | `0x28f05d65fccfc8d99b0dd5ff1c958e7a44775a9ecb48e1d5d18928e6ce1ba6dd` | Benchmark-only direct call; consumes parent and registers a 50000 residual bid with inherited metadata. |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) consume residual | 45853 | 17358632 | 0.000000795945353096 | `0x342df374c007ec8f8003c8fc40a6b8edaae08c55e419fb231c29387faf01e75c` | Benchmark-only direct call; fully consumes the previously registered residual bid. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 158313 | 15195436 | 0.000002405635059468 | `0x720e6f77706b0eb041a4f1dc633344f97794245841a4390650890b3509d0efd8` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 158301 | 13316054 | 0.000002107944664254 | `0xa0128409b68f2ba1c6a0754a90f768357f953a187f2acea7f3df1cbe57ef36c6` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 158301 | 11669114 | 0.000001847232415314 | `0xffb1564ec7834eb2b9386d8f980db46c07ea556686273c0364c24fbf42f9e2ba` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 158301 | 10225869 | 0.000001618765288569 | `0xa066cbeb588bbfa13a5e99e3e28a17c724232961acfe90bf7089a55431f5ee19` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 158313 | 8961126 | 0.000001418662740438 | `0xcc727fe0cb0763cb74504dd02c1f4c185fcf4b9838b8365994207600285f6237` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 158301 | 7852808 | 0.000001243107359208 | `0xcd3174881a52062a27b7ed55df095a257feb7763fb85218bb0fdec5f9e014204` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 158313 | 6881567 | 0.000001089441516471 | `0xfb5eac0d16d4aa8c83a28ae7dd5e21485b5ba0fafb0d7a1c7ac807391ab97946` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 158301 | 6030451 | 0.000000954626423751 | `0xa2eabf087bed921a3ee3f13583d05c2317d32cf7cfc715bf613e056f62e28c9c` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 158313 | 5284600 | 0.0000008366208798 | `0x8e69c646f9a9defb1827622a7327164ff1c4069892c9ea7241db441ff0379ee1` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 158289 | 4630997 | 0.000000733035884133 | `0x50e5f1ef206f2f3780b4caf5eff7c548961467f2d27281094e3cd21cdc433c40` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 158313 | 4058232 | 0.000000642470882616 | `0x1148ca0ab9e4a9c02a925b64db623a92e072caf6ebdbe0a992c92d8a8c612b98` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 158301 | 3556308 | 0.000000562967112708 | `0x8d31d44feca08e85cb6aff1bbba3ab44eaefdbbf285ae88e0a4c54d5b4446b86` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 158313 | 3116462 | 0.000000493376448606 | `0x48deca164897bf1ebe775ecad4255d02339690d90e0b025e73a72f44bdef1e1d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 158301 | 2731016 | 0.000000432322563816 | `0x2d67326d0e979db695b5c1d2921407b5c0dc48df2cbf8c079f408adf7bbf7ce4` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 218138 | 2393242 | 0.000000522057023396 | `0x7316739a4783703d5aaf0d7e7841bed92059d3f51d3e5d1fb69507638e252503` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 390327 | 2098437 | 0.000000819076618899 | `0xb1372e956620a40a244e036468dc4ab1ea636629183ecea8920de257a37b8579` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 906997 | 1842958 | 0.000001671557377126 | `0xf2791c91352c18f77c9c3350ee4d19af72ec1bbd223ee091b8456d73f25d4b11` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] bid 1 | 178417 | 1626519 | 0.000000290198640423 | `0x91e85ec03560c14e4f7a72c7ecfeb503d8adbbc29adc5da26551a4100f5b2c3d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] offer 1 | 158301 | 1425622 | 0.000000225677388222 | `0x2889e16904da27cb51cefcfe50321590d9c7e85f0382051dac3079e2710ec171` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred buyer-only | 219621 | 1249300 | 0.0000002743725153 | `0xe2c06b73432b0f7c4a3800a1aac2f1f3c6490a98e63dff5683cfe6b6b8e00ed1` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 1 | 178429 | 1095425 | 0.000000195455587325 | `0x64d9abf74ebc8183c2f5b33129cc643f292e2dcd30eec43ab2d68b0a5aaea8d9` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 1 | 158289 | 960125 | 0.000000151977226125 | `0x84ce8833af6b5a612ea90de8d4b40ca14f3c0afde1df2849927c6c16a05840a0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 2 | 178429 | 841377 | 0.000000150126056733 | `0xf2b6e8bc1f067312a245eabe704e8ff4b4527c1ffad639accd84e50f7d0f86b1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 2 | 158301 | 737456 | 0.000000116740022256 | `0x20a61564c364f88c1f96376e654c8cf923d29a353aed74e53f7ecbf27d1b6bb1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred buyer-only | 393317 | 646248 | 0.000000254180324616 | `0xc6ae3bf9a306e42531409b860e88d7a8bbbe66b2d7faf66d42400b46f34d6af5` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 1 | 178417 | 567585 | 0.000000101266812945 | `0xf9161ebdeb95a44dc4bd38db3fc8a39ee151b6ee6d89fa6b1fc8cca75a651423` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 1 | 158301 | 497481 | 0.000000078751739781 | `0xc2afa71ac74b151e57dbbabe9776b0424a4f3f3e1d532764c877fe5d535d1b41` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 2 | 178417 | 435953 | 0.000000077781426401 | `0x2982ffadb757f91c195fa976d5074bb040586726bf2724353a06e4419f1b23de` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 2 | 158301 | 382108 | 0.000000060488078508 | `0x2d55aa52733f91931081152bca3711ec059d4453268b623b59562fba7c2899b1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 3 | 178429 | 334850 | 0.00000005974695065 | `0x51c098ab6b4f5d2f30eb1157c59179d81588c6141862556ab1b2552796747606` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 3 | 158301 | 293492 | 0.000000046460077092 | `0x8d8f71bdb113fc773efa7874c7f338b71e6201c8890cf9289983d9007fff9728` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 4 | 178429 | 257193 | 0.000000045890689797 | `0xff38168c1b65b9a4ecc8c32f1e8bcb878ffa9864ce0a728bbde5be13df5cf762` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 4 | 158301 | 225428 | 0.000000035685477828 | `0xd238d578d093be4093b523616b48e8f0e449dbde2fe497064f0373f050a54a7a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 5 | 178429 | 197547 | 0.000000035248113663 | `0x4a8055ec17014540b5c6630fda9f3d2e12233b27e4c4a8090823012b1529321b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 5 | 158301 | 173148 | 0.000000027409501548 | `0x8da3a579a244aa6b9a817933d519f08d7e21923f30812db089eb9f79e82cd7df` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred buyer-only | 914460 | 151734 | 0.00000013875467364 | `0x8593b1ff9263977f331b9d1e1b284a915474f106d011035eff504fdd6b770679` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] bid 1 | 158313 | 133924 | 0.000000021201910212 | `0x456ff3b53863732a5dc6fb95046f468d87a8be1d11b1c6a88638a269e2ab8dc5` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] offer 1 | 178417 | 117361 | 0.000000020939197537 | `0x5179cac73a015623475c7be0f564d90d9e7a94a8e098f234b53b79d5a88725bc` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred seller-only | 239562 | 102866 | 0.000000024642784692 | `0x53461d76f0d74f874ea819219702a14a632f55f15eeb6d146d94db6a05f7a451` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 1 | 158313 | 90213 | 0.000000014281890669 | `0x2f50b1b93f1a16421249f34a5f86fdcba4461a263188e659ee957bd00c4de692` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 1 | 178417 | 79057 | 0.000000014105112769 | `0x1724f73b6e03185ce7399ced150d5e3787c0ad200966e70d55eae5982859c86f` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 2 | 158313 | 69292 | 0.000000010969824396 | `0x83952dd21945d1938c077071ce211b3da4ac1ed27ebc62544465003c0f43f889` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 2 | 178417 | 60723 | 0.000000010834015491 | `0x23cf7b670a472a7fea9aa2a517ce10feb0a2be82377155350852b805d6323ef4` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred seller-only | 433187 | 53223 | 0.000000023055511701 | `0x8436017ec88a1d5d065e6d619c50d8f6b214223f1e0823f9b49b2bcf0f1b3678` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 1 | 158313 | 46763 | 0.000000007403190819 | `0xf26641dd95703beb2e085cd4317390291cf27fc4efdedaa9185500714528fad8` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 1 | 178417 | 40980 | 0.00000000731152866 | `0xb2ac8e0869dd093ebbdf8f2a64a9fec6efc3fe5f5082075cde5275ef4ecc9733` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 2 | 158313 | 35919 | 0.000000005686444647 | `0x1f30aac01df3c1f8b7de9388443e29e733f83cd4bf57b26dfcd69faa9d567fb0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 2 | 178405 | 31477 | 0.000000005615654185 | `0x9e1fbc7f6fd7745fb35ecac067d3c21cddf799260e470255e1fce45af8336dae` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 3 | 158313 | 27590 | 0.00000000436785567 | `0x76382c239fa566da36d5c19739b1ba02b98981507485ae30253ad81e3c8ea11a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 3 | 178417 | 24178 | 0.000000004313766226 | `0xa702d690fdce75bfec706b0ac170df3c5b55b6ca1fd643d70c7220e4892cb8a7` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 4 | 158313 | 21192 | 0.000000003354969096 | `0xfdc3d921cb8df6593f980f783a3b9597702b54634b26f40c3de7525139e1f104` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 4 | 178417 | 18571 | 0.000000003313382107 | `0x44c0db0ea7cfa14a2f88ebd4b96fc29b725029ad2c9dbcf3e06fb6b1d95be15b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 5 | 158313 | 16278 | 0.000000002577019014 | `0x7445a522e026c81a4df1361297b646874e4ba1ed9f8008a15ef7c8d2c5460ccc` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 5 | 178417 | 14266 | 0.000000002545296922 | `0x4ae7569b6540da80750f0bb06000716df890a64cbcd386acea01dc8a414bab9d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred seller-only | 1014129 | 12504 | 0.000000012680669016 | `0x864cdf00b8f3425f0c8c371114a4c28bb879745f38e93e28509abba978ffaa99` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] bid 1 | 178429 | 11047 | 0.000000001971105163 | `0xe9ec47706f624fffd38bd2b4a45013dd691babbe51b1cd5f83b1ea070e782c4f` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] offer 1 | 178417 | 9683 | 0.000000001727611811 | `0x3634b97d252d5de19383e2461425955c90a2d83469b8ca2bc9e2b9c18db31225` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred reciprocal | 239969 | 8488 | 0.000000002036856872 | `0x43de4331169e6cf87c3c3d57764c695e137b8c1a42d4b4c8d3ae2029a21570c0` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 1 | 178429 | 7445 | 0.000000001328403905 | `0x168dc96408ea40cce005fe74a7d003b530f20179c7e34489ac0e65f53f53f052` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 1 | 178417 | 6526 | 0.000000001164349342 | `0x1e5455f9f51cc51953ead536a08d95310f38f41a9e65126502ed743f739c0cc7` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 2 | 178417 | 5721 | 0.000000001020723657 | `0x2da842095fc64f037c031408299839892702b808591b08e41012952e168264e4` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 2 | 178417 | 5015 | 0.000000000894761255 | `0x3cd827bf10aae70660ab7ea11d8f4f1f4cbe3db9b3230304dcc262d989509cd6` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred reciprocal | 433989 | 4396 | 0.000000001907815644 | `0xbabc871b951e08348b610b34346b41807a167a08e905859c604f470c5a21841f` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 1 | 178429 | 3863 | 0.000000000689271227 | `0x4e5852bc01c62eb79821a01cf4f39a07e84709ecf20dece122ec9f6c3481baa5` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 1 | 178417 | 3386 | 0.000000000604119962 | `0xa4ff575e0ae4ab5f79fa581d18bb24a113a9c6e2f2586644e245d03d45c78583` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 2 | 178429 | 2969 | 0.000000000529755701 | `0xa67b83fc5844aa6b3e0752a66083f81a48ece5b78635f9f278b3e08bd27a8ff6` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 2 | 178417 | 2603 | 0.000000000464419451 | `0xdff0477e094473f84a2d9dbf30746bf60635608b432a8e7396e75932b71aa6c1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 3 | 178417 | 2281 | 0.000000000406969177 | `0x3fa7c386ca0e530f5353c347bbfd7db10eceec1e4644047366f764aadad92c42` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 3 | 178417 | 2000 | 0.000000000356834 | `0x36572d0aef265b0e721d3ec790b88c5be809925be251ba5cbc6bba61b19ad2f7` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 4 | 178429 | 1754 | 0.000000000312964466 | `0x29a16df0c313df9a9d1685d4685988745a7f42618d273e5d5db4827acff2ff62` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 4 | 178417 | 1537 | 0.000000000274226929 | `0x33cfcfa00a012d83676f41d983d343a92fbcf6778a4758448b94714ce8d7fcfd` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 5 | 178417 | 1347 | 0.000000000240327699 | `0x9140b6a7e28fb0280bd4e5c8bd0e76b7c29f6ef71f4aef79b965b2753e5801ce` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 5 | 178417 | 1182 | 0.000000000210888894 | `0x72c4f67523e3aa1a6d6100d0c8322eb9993853da4d2ddcec6b1956b3781f7a6e` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred reciprocal | 1016140 | 1036 | 0.00000000105272104 | `0xddae9785e8f58f977154a289996ed78cee40abb3b790a7e1c8c60f61f876ec63` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80362 | 916 | 0.000000000073611592 | `0x6f62f58d0c0b1b70535495fe506ca01e24809858f52a39880e07cbf67d08fffe` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 31285 |  |  | view estimate |  |
| View estimates | MarketController | marketsExist(3 markets) | 36192 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29062 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 36899 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28892 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28871 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Standard (matchType=0) and preferred (matchType=1: buyer-only, seller-only, reciprocal) batches each fill 100000 of a 100000 bid and a 150000 offer per match, at price 12000. Each match includes on-chain creation of a 50000 residual offer.
- Preferred benchmarks set both effective rates to 12000. The counterpart without a preferred partner uses its normal rate when no preferred rate is supplied. Differences include calldata and storage access costs, not just validation instructions.
- Direct `settleOrder` rows use a benchmark-only settlement role. Production `settleBatch` rows already include these internal calls; do not add their gas again when estimating a trade.
- Totals include all benchmark fixtures and alternative cases, not the cost of a single production deployment/trading workflow. Local fees are illustrative, not remote-network price estimates.
- Mainnet/Volta values depend on live gas price at execution time.
