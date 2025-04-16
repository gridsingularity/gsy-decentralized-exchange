export const ETHEREUM_DID_REGISTRY_ABI = [
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "revokeAttribute",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "bytes32",
        "name": "name"
      },
      {
        "type": "bytes",
        "name": "value"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "view",
    "payable": false,
    "outputs": [
      {
        "type": "address",
        "name": ""
      }
    ],
    "name": "owners",
    "inputs": [
      {
        "type": "address",
        "name": ""
      }
    ],
    "constant": true
  },
  {
    "type": "function",
    "stateMutability": "view",
    "payable": false,
    "outputs": [
      {
        "type": "uint256",
        "name": ""
      }
    ],
    "name": "delegates",
    "inputs": [
      {
        "type": "address",
        "name": ""
      },
      {
        "type": "bytes32",
        "name": ""
      },
      {
        "type": "address",
        "name": ""
      }
    ],
    "constant": true
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "setAttributeSigned",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "uint8",
        "name": "sigV"
      },
      {
        "type": "bytes32",
        "name": "sigR"
      },
      {
        "type": "bytes32",
        "name": "sigS"
      },
      {
        "type": "bytes32",
        "name": "name"
      },
      {
        "type": "bytes",
        "name": "value"
      },
      {
        "type": "uint256",
        "name": "validity"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "changeOwnerSigned",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "uint8",
        "name": "sigV"
      },
      {
        "type": "bytes32",
        "name": "sigR"
      },
      {
        "type": "bytes32",
        "name": "sigS"
      },
      {
        "type": "address",
        "name": "newOwner"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "view",
    "payable": false,
    "outputs": [
      {
        "type": "bool",
        "name": ""
      }
    ],
    "name": "validDelegate",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "bytes32",
        "name": "delegateType"
      },
      {
        "type": "address",
        "name": "delegate"
      }
    ],
    "constant": true
  },
  {
    "type": "function",
    "stateMutability": "view",
    "payable": false,
    "outputs": [
      {
        "type": "uint256",
        "name": ""
      }
    ],
    "name": "nonce",
    "inputs": [
      {
        "type": "address",
        "name": ""
      }
    ],
    "constant": true
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "setAttribute",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "bytes32",
        "name": "name"
      },
      {
        "type": "bytes",
        "name": "value"
      },
      {
        "type": "uint256",
        "name": "validity"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "revokeDelegate",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "bytes32",
        "name": "delegateType"
      },
      {
        "type": "address",
        "name": "delegate"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "view",
    "payable": false,
    "outputs": [
      {
        "type": "address",
        "name": ""
      }
    ],
    "name": "identityOwner",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      }
    ],
    "constant": true
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "revokeDelegateSigned",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "uint8",
        "name": "sigV"
      },
      {
        "type": "bytes32",
        "name": "sigR"
      },
      {
        "type": "bytes32",
        "name": "sigS"
      },
      {
        "type": "bytes32",
        "name": "delegateType"
      },
      {
        "type": "address",
        "name": "delegate"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "addDelegateSigned",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "uint8",
        "name": "sigV"
      },
      {
        "type": "bytes32",
        "name": "sigR"
      },
      {
        "type": "bytes32",
        "name": "sigS"
      },
      {
        "type": "bytes32",
        "name": "delegateType"
      },
      {
        "type": "address",
        "name": "delegate"
      },
      {
        "type": "uint256",
        "name": "validity"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "addDelegate",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "bytes32",
        "name": "delegateType"
      },
      {
        "type": "address",
        "name": "delegate"
      },
      {
        "type": "uint256",
        "name": "validity"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "revokeAttributeSigned",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "uint8",
        "name": "sigV"
      },
      {
        "type": "bytes32",
        "name": "sigR"
      },
      {
        "type": "bytes32",
        "name": "sigS"
      },
      {
        "type": "bytes32",
        "name": "name"
      },
      {
        "type": "bytes",
        "name": "value"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "nonpayable",
    "payable": false,
    "outputs": [],
    "name": "changeOwner",
    "inputs": [
      {
        "type": "address",
        "name": "identity"
      },
      {
        "type": "address",
        "name": "newOwner"
      }
    ],
    "constant": false
  },
  {
    "type": "function",
    "stateMutability": "view",
    "payable": false,
    "outputs": [
      {
        "type": "uint256",
        "name": ""
      }
    ],
    "name": "changed",
    "inputs": [
      {
        "type": "address",
        "name": ""
      }
    ],
    "constant": true
  },
  {
    "type": "event",
    "name": "DIDOwnerChanged",
    "inputs": [
      {
        "type": "address",
        "name": "identity",
        "indexed": true
      },
      {
        "type": "address",
        "name": "owner",
        "indexed": false
      },
      {
        "type": "uint256",
        "name": "previousChange",
        "indexed": false
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "DIDDelegateChanged",
    "inputs": [
      {
        "type": "address",
        "name": "identity",
        "indexed": true
      },
      {
        "type": "bytes32",
        "name": "delegateType",
        "indexed": false
      },
      {
        "type": "address",
        "name": "delegate",
        "indexed": false
      },
      {
        "type": "uint256",
        "name": "validTo",
        "indexed": false
      },
      {
        "type": "uint256",
        "name": "previousChange",
        "indexed": false
      }
    ],
    "anonymous": false
  },
  {
    "type": "event",
    "name": "DIDAttributeChanged",
    "inputs": [
      {
        "type": "address",
        "name": "identity",
        "indexed": true
      },
      {
        "type": "bytes32",
        "name": "name",
        "indexed": false
      },
      {
        "type": "bytes",
        "name": "value",
        "indexed": false
      },
      {
        "type": "uint256",
        "name": "validTo",
        "indexed": false
      },
      {
        "type": "uint256",
        "name": "previousChange",
        "indexed": false
      }
    ],
    "anonymous": false
  }
]