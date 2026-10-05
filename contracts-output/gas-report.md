# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-05T12:13:37.868Z
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

- Deployment gas: 7028732
- Deployment fee: 0.005643246332166197 ETH
- Role setup gas: 339814
- Role setup fee: 0.000116228829351983 ETH
- Mutating call gas: 18193680
- Mutating call fee: 0.000127431338100059 ETH
- Deployment + setup + mutating fee: 0.005886906499618239 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 584915 | 981513999 | 0.000574102260725085 | `0xbe812ee0fd9c0d4800d0ca1d71dc09c2e692d638b0058641f12521c6812cf252` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696079 | 863608934 | 0.000601140043169786 | `0x8064f638f0e9d85a2602882da093409a517ba4c205860e0cf773b256c09b34b7` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1450498 | 760667319 | 0.001103346424874862 | `0xe5ee02bdffad6ffda1bf549bf9900dc8285006675215b277d582970d757e82cf` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741598 | 674778458 | 0.000500414354895884 | `0xe76fde5bedddeb5b746a090e2966cd1cf696e338a1928f1dd2090930c1b262b3` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1484478 | 594601271 | 0.000882672505571538 | `0xd6ead034e2460960ad093a5b8d8d8701132682bdb7e0ccff667913e76e1a92c7` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718850 | 527631716 | 0.0003792880590466 | `0xde2c9599187aa5d5d9d3fac792648a207e819eece6f875419ac4dfd8bd5fef6b` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 464838486 | 0.00002632612765461 | `0x9781ef04c5eaee9435146b6203092f93496154d6b2bc3a4866ad0202688a29ae` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56658 | 406953060 | 0.00002305714647348 | `0x390428ffd5c24dba9a968647bf2a5c51b48f61cd2a7eeff3776bf81a4cab5785` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56658 | 356276071 | 0.000020185889630718 | `0x0186cdb28722b9abe6fed5eae66f7f95455a63305b71df5c2cc681c0172b7ec3` | Benchmark-only grant used to measure updateStatus and settleOrder directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 311909779 | 0.000017665322243444 | `0x03ef3475ddacda9db68b6972455c074acb664634aaf760839ba6323bbb375680` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 273068268 | 0.000015465494426448 | `0x646f5d9d1f3ad8598b348dab614dcfbaf7571faf65645fe7a80bd9301efff1d3` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 239063613 | 0.000013528848923283 | `0x27231ec72ad16a2253ad50c60aa9dcac511648f6a7d9b2240bf3f9fdb02a6f4b` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 209293402 | 0.000011276309912956 | `0xc301875dc836d3f28dda5d3a79b36178906676cccc128c5db83e944d82a89705` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 183225696 | 0.000009871834049088 | `0x33a51a454930ce643391908ce560afe578864e66a25b1a7ffab4bc8545b78544` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 160404750 | 0.00000867484928475 | `0xed53b48c90807876134b9d8acbce7f36a690f1e55c4e9e5c568775265f1aedf7` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 140426447 | 0.000004517378373543 | `0x3ed8f6031fd392f956f1c0f974984cf2b31aef2457b95d3b36c659a5dc9f3834` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 122910787 | 0.000006609404660138 | `0x9a88989061ff7bc15bc47ed938729c2972dcae6d76e84aa63b11edeee1b46868` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 107602017 | 0.000003428415465654 | `0xf0e201099b4932578ad9d07483bc686991ad9a4c21a900cbbc68ed3c092b14d4` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53056 | 94180336 | 0.000004996831906816 | `0x898e849bc241f4fb6f8a7c7a8530c69e2138e2437f8e9cf031600acb6b79f746` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31144 | 82449435 | 0.00000256780520364 | `0x7a47042787e8ab821207d165d581f822803fb097ce8787fb4441dcc7bf829e08` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53056 | 72164655 | 0.00000382876793568 | `0xa83ff3244f564f42fbd64ab62d579b349260af7cd6412f0306b8cab5b3d3b9dc` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 156055 | 63175980 | 0.0000098589275589 | `0x0fd6888f800afb4b5b71a2284d4b54863d2679e142c5d216bf1fc165dfc1f445` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 156043 | 55361140 | 0.00000863871836902 | `0xd905f5d9ec3377fa8cb74fb59a23dfed5a5864f7c9ffc7f33795f902c0353289` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 156055 | 48512988 | 0.00000757069434234 | `0x4442c17dd9da81b14030504805b7beac14862be8d92979c526e896bea18a7a84` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51378 | 42511953 | 0.000002184179121234 | `0x7f9c818b889e9bb6fa157edd8143cc0321bde0051837ad8be3936d1fdc2b967b` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 156055 | 37216161 | 0.000005807768004855 | `0x0bce8c105de6c585fb553c1d9bd05dbf7752649b6a16ab89cec68117a102391c` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35972 | 32612540 | 0.00000117313828888 | `0x1d294933c4eba736ff0f6d16c68faccb323e22a8741ccc90e13f77b139b45d00` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleOrder benchmark | 156055 | 28545749 | 0.000004454706860195 | `0xb47c842abe7782a05b6896aa9e9c05851d3dfdf66021b540dd7ff5d2299c7e81` |  |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) partial fill | 145074 | 25014654 | 0.000003628975914396 | `0xeab327a2a840f8fd2cd5ff0250a8f0853ebbd402798cdd53b4e588748c54a9c5` | Benchmark-only direct call; consumes parent and registers a 50000 residual bid with inherited metadata. |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) consume residual | 45853 | 21918064 | 0.000001005008988592 | `0x9aa5b80d21560325458d013666637c1d603b3676d9564374b0a924c335106384` | Benchmark-only direct call; fully consumes the previously registered residual bid. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 156055 | 19186682 | 0.00000299417765951 | `0x66110bbeed5fe4dcbdaafde29c8cce6a2535dceb3784d71d4e09abda8007446a` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 156043 | 16813299 | 0.000002623597615857 | `0x9440daad31f90682e80603a4385e6b223aff5b434c04a8f3d6fc0765c9cbc196` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 156043 | 14733500 | 0.0000022990595405 | `0x3cca2d41e5f6a4ae5a3e824bd17ff1f50a7ff6a16ad76ca8691d97304c56d691` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 156043 | 12910972 | 0.000002014666803796 | `0x151ca78bde9eb803acf84bc9a47b1c86eb6fa9ff0784389b8a0f34e31f6fbd5e` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 156055 | 11313890 | 0.00000176558910395 | `0x46ecaf44708f08e1e77b99ccaccacadd41067a59d12476c652c117169a2a026c` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 156043 | 9914368 | 0.000001547067725824 | `0xfbd50d5927f49467b351eae01021e69c6f5da3452fc4380ae46010e70131bca5` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 156055 | 8687965 | 0.000001355800378075 | `0x1037506e3b6002c50861cec34fa9cf47772293391c2902310e55fadd41a4aeaf` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 156043 | 7613268 | 0.000001187997178524 | `0xeab415a975fe451c4bfdc08683915ebd3c0b38d0dfa4b5daefea1483c7241753` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 156055 | 6671510 | 0.00000104112249305 | `0xd20db3a36e2b70ad54cad166b894404a3d7993975a545fdd7a7c344d7ea5828d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 156031 | 5846248 | 0.000000912195921688 | `0xb767bc14173c37680cf5a8d0fb18a4f2a8a6ecc2563a46ef1d77ab6da42f0c6f` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 156055 | 5123069 | 0.000000799480532795 | `0x5dfae5fa496bfbd3525e63a259a35a16a1aebf9a748b19da51b0e814c31b82ec` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 156043 | 4489347 | 0.000000700531173921 | `0x51a436a049013804407692bd477a2e01834338068635087ef122fd969f8c57c0` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 156055 | 3934018 | 0.00000061392317899 | `0xaa13beb34d69377f6388cb24c6b338a57f3f8a863670976e326f32041413a1a8` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 156043 | 3447382 | 0.000000537939829426 | `0x83efcace55d5fc0ed0377c32fd8562828de91ab37db7069e03036584621a026a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 211494 | 3020943 | 0.000000638911318842 | `0x82215f344c3b724938a41e03c3281d55defccc73509224937d454e4f17111dd2` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 382708 | 2648650 | 0.0000010136595442 | `0x8658fd925b635cad10278dd1e997d05e6f28ad2094530a0a3f50bef7c20efe3c` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 896456 | 2326017 | 0.000002085171895752 | `0xf7da2a0c72b16349f9b15ba96a7d71fd1831cf0160efdba1607c201bc280f0fa` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] bid 1 | 176159 | 2052642 | 0.000000361591362078 | `0xe783cc42cba75a32b71a4fb6b7bb99053ddc43a77a4628242acdffd53e90ad14` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] offer 1 | 156043 | 1799075 | 0.000000280733060225 | `0x2e7bb496fd679048d7c2531ea134c040acfc91b1a9bb9fdc6e766278e3aee797` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred buyer-only | 212919 | 1576530 | 0.00000033567319107 | `0x5639f986804f5d1bf3e56a7e1d092664a8d93a2d9f2bad8dd18d2fb808b55e4c` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 1 | 176171 | 1382262 | 0.000000243514478802 | `0x7cdb5de62690b88fe336d4a7a3b229e640e3f393537ed7682ff99e999b97c262` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 1 | 156031 | 1211509 | 0.000000189032960779 | `0xd796a0571940eb2135e427b284e83601889b8c826f0ac7d5f842e8e0e63c4079` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 2 | 176171 | 1061646 | 0.000000187031237466 | `0xc80bea9ef9ef1888f452097287c34ac1edfc522961753b46461db0b2f9089581` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 2 | 156043 | 930500 | 0.0000001451980115 | `0x2059c54c104427784ebd6035d9cd1e6ab0bb27b98cdd501e0c44bfc29b1721cc` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred buyer-only | 385582 | 815397 | 0.000000314402406054 | `0xa9d4f42dd4a6094cfa223ad74e7eae88c13fe894b8212b3517581fd4d007a128` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 1 | 176159 | 716094 | 0.000000126146402946 | `0x23dea5e2a27dac199254708459ba4ca20707865da9f330fb2ce011bb7be0a0f3` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 1 | 156043 | 627634 | 0.000000097937892262 | `0x90b52b4a1ebcbce4d6775217e49f390984cd73bc0f5eed408fa78cf4c6783224` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 2 | 176159 | 549997 | 0.000000096886921523 | `0x52df9427a7478df1f45adff88c2d69d658bd660e63710f63e8b2faf2f054d97d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 2 | 156043 | 482056 | 0.000000075221464408 | `0x1d2b2d8bcb1088090cec142e4b91f667c0182a129c665a8b16dafb1a14877e35` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 3 | 176171 | 422426 | 0.000000074419210846 | `0x1fe4dd0ca5285ffc3ab22a5a6513d89430062bcc6e323802c1057ff05e91bb01` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 3 | 156043 | 370243 | 0.000000057773828449 | `0xa46768120a69f0e96c1d645b6d31804a5467beac9380faf061c0d3e677867053` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 4 | 176171 | 324445 | 0.000000057157800095 | `0xe230aa7c21d01d899622c08ef7ec0ee0b7d6dc2a804464a804fdaadb19906582` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 4 | 156043 | 284366 | 0.000000044373323738 | `0x0e6ca9b47219a8c1be2ca9d026e775b238cf35a1c08bef72dc6dcc76136b469e` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 5 | 176171 | 249191 | 0.000000043900227661 | `0xd56ad7c3101f85c91b3276c9a630de01568d6445e7f89ae551f7647c69ee515a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 5 | 156043 | 218408 | 0.000000034081039544 | `0xa5d49a8780e0f53049ee8f90b94317f648a586c85c61156b1e3fb81b2874ba0d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred buyer-only | 903629 | 191392 | 0.000000172947361568 | `0x052c1c63963c79b8ca5ea52458a6bcde9722f2f41ea2b542c3581571975726d5` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] bid 1 | 156055 | 168910 | 0.00000002635925005 | `0x8f26617ae77febcd0a8ee6c829274af34ea34c9762ec33b4b65ef66e3b7ead10` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] offer 1 | 176159 | 148017 | 0.000000026074526703 | `0x03e35b04c0e7f9fd25171038aad90e41dd22fa8a233c30170d84f454784ccf35` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred seller-only | 232860 | 129732 | 0.00000003020939352 | `0xc41822a1256cbf7a4e9e8cbd7664bf7f5b06b8ea9c9c1f9895045a76eea2578f` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 1 | 156055 | 113768 | 0.00000001775406524 | `0xc7ef6ff633502301d339d302deb435ace47fa883a1a98da451ff810ca1d2ab74` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 1 | 176159 | 99695 | 0.000000017562171505 | `0x472de54be67ec83d3faad65210b9fecb8b01758278a4f0f4f04b852305466c83` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 2 | 156055 | 87381 | 0.000000013636241955 | `0x82b0802de0730ec22657782823e24ff2381346a6a5dc721d1f80f9b00ee26c06` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 2 | 176159 | 76572 | 0.000000013488846948 | `0x774250e450cbd474cbd97fc2d97ba82bd4c9fbb68b2d4b18f1c6d8f7b499fa21` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred seller-only | 425452 | 67114 | 0.000000028553785528 | `0x12ef6e0abe799d453e2dcdbe5e6ddd68eb37634a5d8a1ebf4c231747ca1e28d7` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 1 | 156055 | 58964 | 0.00000000920162702 | `0x7195236376817e0db206ccf2b06108763526cc004656bf01c37367c1fc6c06f8` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 1 | 176159 | 51670 | 0.00000000910213553 | `0x70b7bdff1aaf4b89d90b23f889b93f6dc7057418b512d7f12b983770c444ecee` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 2 | 156055 | 45287 | 0.000000007067262785 | `0xfc859acfe433275786cb6475b9b922c9e70363435b22d47ae1a13afe8016f1af` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 2 | 176147 | 39686 | 0.000000006990569842 | `0x02a5115585f9baee680a149d2d88ade9b9d0ba22de3d254aef901d2fb15965aa` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 3 | 156055 | 34784 | 0.00000000542821712 | `0x510c45cb6bf770b9a248eb8d177389ebd6bdd8d3ec9b786ad5991c3261e4b033` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 3 | 176159 | 30482 | 0.000000005369678638 | `0xadf5d064592798add517d0196fb468990935994fc00b778a3e6ec163fd33d90a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 4 | 156055 | 26717 | 0.000000004169321435 | `0x70d513a970106737f5a65014d8db59ab2a76a19b8cd9b4c85f66025077c99978` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 4 | 176159 | 23412 | 0.000000004124234508 | `0x06049ce2f0636f488d70fc90599d11ab5390b5e087ec67e32f0d625bf22ba836` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 5 | 156055 | 20520 | 0.0000000032022486 | `0x87027109ae2a9d0d01c4b971903736a6ebdd21f7eb22ac224ad7ab9638fe2a08` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 5 | 176159 | 17983 | 0.000000003167867297 | `0x950d2e4223602fd1179038ec381eb25bc1a883a548c9345191990ab341f577c4` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred seller-only | 1003298 | 15762 | 0.000000015813983076 | `0x282c5324ab8cf5cbedd99d210cd789caf487fd360a4a1f70c78d832ff5cd0588` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] bid 1 | 176171 | 13924 | 0.000000002453005004 | `0x318d16f8e1c9f76e1c5862c70317b219332c822b632c2b274b5c37ea6e40166b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] offer 1 | 176159 | 12204 | 0.000000002149844436 | `0x478ed6d433f64a6dbcef17aadc7ffa9cde012f6cf7951978e349c6f4eb89045d` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred reciprocal | 233267 | 10697 | 0.000000002495257099 | `0xccd91a607de0f5e8db4895ddc010ffbb05c0be994d8a2ae87d929354f841f995` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 1 | 176171 | 9380 | 0.00000000165248398 | `0xf664d9e00afea578bc8584e48fc4d3b9070060363122f845e83d84506fbce137` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 1 | 176159 | 8222 | 0.000000001448379298 | `0x61cfddac95e4a6b9246a859a6b3e4052e03b8cb14db11ddef33d3fec398ee8ce` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 2 | 176159 | 7207 | 0.000000001269577913 | `0xf008d79cc8876e3480fb6568997cfb98ca32b0201e47123a853d89f78ec5a7fa` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 2 | 176159 | 6318 | 0.000000001112972562 | `0xcc13e87c0713c38198fdab1cc82608eb6dde6a9b31bc0b20b9d9973e7c81c546` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred reciprocal | 426254 | 5538 | 0.000000002360594652 | `0x8fc74d2ed0ff314b2e257add7c1e8686a9e0c271e45f788ad9607152a938a666` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 1 | 176171 | 4866 | 0.000000000857248086 | `0x5510af8b7299c9c88108c2b2b39b343abae47f12ab7adc41e05d64dfddf6da68` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 1 | 176159 | 4266 | 0.000000000751494294 | `0x5ebd47c1f0cc76680cd1f840d092cf09ea711246ddda4c2c8ed420c2c4af1c24` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 2 | 176171 | 3739 | 0.000000000658703369 | `0x5fdd42843d990871f565cf881f078aa857e0aa4b68610bda162850c11fc0c685` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 2 | 176159 | 3277 | 0.000000000577273043 | `0xffceb82898008334fef753b99703595b35a7c7923598e5bcd60aecc5248570bb` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 3 | 176159 | 2874 | 0.000000000506280966 | `0x9f5afa0b4f5c8545e36dfd96e7d131804025bc2c15dc67b884cfe9b158179cb6` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 3 | 176159 | 2519 | 0.000000000443744521 | `0x49e3d18a036cf5e2cd6fc3d8980acc140a0f5eb1c1235cd2f6eec8cdf319446c` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 4 | 176171 | 2209 | 0.000000000389161739 | `0x20a83b540c0d54c924ffb444241c40a3b145c7fd032877c913bd5444524b7b55` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 4 | 176159 | 1937 | 0.000000000341219983 | `0xfcd785763288778bfec127bf621f47347d538b3a756b5745c02ff003f36a3b79` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 5 | 176159 | 1698 | 0.000000000299117982 | `0x3317aa3b046bece3adffa05e712d9bb909f12a3aebc5301320d0cd8136d72699` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 5 | 176159 | 1489 | 0.000000000262300751 | `0x5bcfe34ad8ecb77a1e1bb29239b993d34e77d6d8e60dd7f817244893482d7539` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred reciprocal | 1005309 | 1306 | 0.000000001312933554 | `0x5344347e66522dd464b4fb10995dd2e2bceaa87e2e6466cef994d82621cc7141` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 1154 | 0.000000000092763136 | `0x1da5d2203c684c2a860708c4ac3c27cc5d8d08f5164c3cb2372a69827c265b78` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 28991 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29062 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 36899 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
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
