# Pallets

The runtime in this project is constructed using many FRAME pallets that ship with the
[core Substrate repository](https://github.com/paritytech/substrate/tree/master/frame) and
 `orderbook-registry`, `orderbook-worker` and `trades-settlement` pallets
that are defined in the `./modules` directory.

A FRAME pallet is compromised of a number of blockchain primitives:

- Storage: FRAME defines a rich set of powerful
  [storage abstractions](https://docs.substrate.io/v3/runtime/storage) that makes
  it easy to use Substrate's efficient key-value database to manage the evolving state of a
  blockchain.
- Dispatchables: FRAME pallets define special types of functions that can be invoked (dispatched)
  from outside of the runtime in order to update its state.
- Events: Substrate uses [events and errors](https://docs.substrate.io/v3/runtime/events-and-errors)
  to notify users of important changes in the runtime.
- Errors: When a dispatchable fails, it returns an error.
- Config: The `Config` configuration interface is used to define the types and parameters upon
  which a FRAME pallet depends.

## Orderbook Registry Pallet

The Orderbook Registry pallet provides a decentralized orderbook management system that allows users to register accounts, insert and delete orders, and manage proxies for delegating order management.

### Pallet Overview

The pallet provides several key components:

1. User and Exchange Operator registration.
2. Proxy account management for users.
3. Order management, including insertion and deletion.

### Storage Items

- `RegisteredUser`: Maps an AccountId to a Hash for registered users.
- `RegisteredExchangeOperator`: Maps an AccountId to a Hash for registered exchange operators.
- `ProxyAccounts`: Maps an AccountId to a BoundedVec of ProxyDefinition for the registered proxy accounts.
- `OrdersRegistry`: Maps an OrderReference to an OrderStatus.

### Events

- `ExchangeOperatorRegistered`: Emitted when a new exchange operator is registered.
- `NewOrderInserted`: Emitted when a new order is inserted.
- `NewOrderInsertedByProxy`: Emitted when a new order is inserted by a proxy account.
- `AllOrdersInserted`: Emitted when all orders in a batch are inserted.
- `OrderDeleted`: Emitted when an order is deleted.
- `OrderDeletedByProxy`: Emitted when an order is deleted by a proxy account.
- `ProxyAccountRegistered`: Emitted when a new proxy account is registered.
- `ProxyAccountUnregistered`: Emitted when a proxy account is unregistered.
- `UserRegistered`: Emitted when a new user is registered.

### Errors

- `AlreadyRegistered`: Returned when an account is already registered.
- `AlreadyRegisteredProxyAccount`: Returned when a proxy account is already registered.
- `NoSelfProxy`: Returned when an account tries to register itself as a proxy account.
- `NotARegisteredExchangeOperator`: Returned when an account is not a registered exchange operator.
- `NotARegisteredProxyAccount`: Returned when an account is not a registered proxy account.
- `NotARegisteredUserAccount`: Returned when an account is not a registered user account.
- `NotARegisteredUserOrProxyAccount`: Returned when an account is not a registered user or proxy account.
- `NotRegisteredProxyAccounts`: Returned when there are no registered proxy accounts.
- `OpenOrderNotFound`: Returned when an open order is not found.
- `OrderAlreadyDeleted`: Returned when an order is already deleted.
- `OrderAlreadyExecuted`: Returned when an order is already executed.
- `OrderAlreadyInserted`: Returned when an order is already inserted.
- `ProxyAccountsLimitReached`: Returned when the proxy accounts limit has been reached.

### Dispatchable Functions

- `insert_orders`: Insert an order with a given order hash for a registered user account.
- `insert_orders_by_proxy`: Insert an order with a given order hash for a registered user account by a registered proxy account.
- `delete_order`: Delete an order with a given order hash for a registered user account.
- `delete_order_by_proxy`: Delete an order with a given order hash for a registered user account by a registered proxy account.
- `register_proxy_account`: Register a new proxy account for a registered user account.
- `register_exchange_operator`: Register a new exchange operator account.
- `register_user`: Register a new user account.
- `unregister_proxy_account`: Unregister a proxy account for a registered user account.

### Helper Functions

- `add_exchange_operator`: Add an exchange operator account.
- `add_proxy_account`: Add a proxy account for a registered user account.
- `add_user`: Add a user account.
- `is_order_registered`: Check if an order is registered.
- `is_registered_exchange_operator`: Check if an account is a registered exchange operator.
