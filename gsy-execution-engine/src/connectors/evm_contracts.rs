use ethers::prelude::abigen;

abigen!(
    TradeSettlementContract,
    r#"[
        {
            "type": "function",
            "name": "hasRole",
            "stateMutability": "view",
            "inputs": [
                {"name": "role", "type": "bytes32"},
                {"name": "account", "type": "address"}
            ],
            "outputs": [{"name": "", "type": "bool"}]
        },
        {
            "type": "function",
            "name": "submitPenalties",
            "stateMutability": "nonpayable",
            "inputs": [
                {
                    "name": "penalties",
                    "type": "tuple[]",
                    "components": [
                        {"name": "penalizedActorId", "type": "bytes16"},
                        {"name": "marketId", "type": "bytes16"},
                        {"name": "tradeId", "type": "bytes16"},
                        {"name": "penaltyEnergy", "type": "uint64"}
                    ]
                }
            ],
            "outputs": []
        },
        {
            "type": "function",
            "name": "penaltyEnergyByTrade",
            "stateMutability": "view",
            "inputs": [{"name": "tradeId", "type": "bytes16"}],
            "outputs": [{"name": "", "type": "uint256"}]
        }
    ]"#
);
