<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://github.com/btn-group">
    <img src="images/logo.png" alt="Logo" height="80">
  </a>

  <h3 align="center">Secret Network BUTT Migration Smart Contract by btn.group</h3>
</div>

<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#setting-up-locally">Setting up locally</a></li>
      </ul>
    </li>
  </ol>
</details>

<!-- ABOUT THE PROJECT -->
## About The Project

This is a smart contract to assist with the migration of BUTT to other chains. It's roles are to:
1. Allow/enforce admin to transfer BUTT to Mount Doom only.
2. Allow users to send BUTT to the smart contract and specify the Aleph Zero wallet address they want it sent to, privately.
3. Collect a fee per transaction.
4. Allow/enforce admin to set the Aleph Zero transaction hash for the order.

<p align="right">(<a href="#top">back to top</a>)</p>

### Built With

* [Cargo](https://doc.rust-lang.org/cargo/)
* [Rust](https://www.rust-lang.org/)
* [secret-toolkit](https://github.com/scrtlabs/secret-toolkit)

<p align="right">(<a href="#top">back to top</a>)</p>

<!-- GETTING STARTED -->
## Getting Started

To get a local copy up and running follow these simple example steps.

### Prerequisites

* Download and install secretcli: https://docs.scrt.network/cli/install-cli.html
* Setup developer blockchain and Docker: https://docs.scrt.network/dev/developing-secret-contracts.html#personal-secret-network-for-secret-contract-development

### Setting up locally

Do this on the command line (terminal etc) in this folder.

1. Run chain locally and make sure to note your wallet addresses.

```sh
docker run -it --rm -p 26657:26657 -p 26656:26656 -p 1337:1337 -v $(pwd):/root/code --name secretdev enigmampc/secret-network-sw-dev
```

2. Access container via separate terminal window

```sh
docker exec -it secretdev /bin/bash

# cd into code folder
cd code
```

3. Store contract

```sh
# Store contracts required for test
secretcli tx compute store snip-20-reference-impl.wasm.gz --from a --gas 3000000 -y --keyring-backend test
secretcli tx compute store sn-butt-migration.wasm.gz --from a --gas 3000000 -y --keyring-backend test
```

4. Get the contract's id

```sh
secretcli query compute list-code
```

5. Init BUTT 

```sh
CODE_ID=1
INIT='{"name": "Buttcoin", "symbol": "BUTT", "decimals": 6, "initial_balances": [{"address": "secret1krq6nl2qdgu66t7ghsner7sr69nz8z8v7z9t3a", "amount": "1000000000000000000"},{"address": "secret1t9ppg25nm2fwds9c2dcfnn2r9gg79vla3j5ppl", "amount": "1000000000000000000"}], "prng_seed": "testing"}'
secretcli tx compute instantiate $CODE_ID "$INIT" --from a --label "Buttcoin" -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
```

6. Set viewing key for BUTT

```sh
secretcli tx compute execute secret18vd8fpwxzck93qlwghaj6arh4p7c5n8978vsyg '{"set_viewing_key": { "key": "testing" }}' --from a -y --keyring-backend test
secretcli tx compute execute secret18vd8fpwxzck93qlwghaj6arh4p7c5n8978vsyg '{"set_viewing_key": { "key": "testing" }}' --from b -y --keyring-backend test
```

7. Init SSCRT

```sh
CODE_ID=1
INIT='{"name": "SSCRT", "symbol": "SSCRT", "decimals": 6, "initial_balances": [{"address": "secret1krq6nl2qdgu66t7ghsner7sr69nz8z8v7z9t3a", "amount": "1000000000000000000"},{"address": "secret1t9ppg25nm2fwds9c2dcfnn2r9gg79vla3j5ppl", "amount": "1000000000000000000"}], "prng_seed": "testing"}'
secretcli tx compute instantiate $CODE_ID "$INIT" --from a --label "SSCRT" -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
```

8. Set viewing key for SSCRT

```sh
secretcli tx compute execute secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3 '{"set_viewing_key": { "key": "testing" }}' --from a -y --keyring-backend test
secretcli tx compute execute secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3 '{"set_viewing_key": { "key": "testing" }}' --from b -y --keyring-backend test
```

9. Check instance creation

```sh
secretcli query compute list-contract-by-code $CODE_ID
```

10. Init BUTT Migration Contract

```sh
CODE_ID=2
INIT='{"butt": {"address": "secret18vd8fpwxzck93qlwghaj6arh4p7c5n8978vsyg", "contract_hash": "35F5DB2BC5CD56815D10C7A567D6827BECCB8EAF45BC3FA016930C4A8209EA69"}, "mount_doom": {"address": "secret18vd8fpwxzck93qlwghaj6arh4p7c5n8978vsyg", "contract_hash": "35F5DB2BC5CD56815D10C7A567D6827BECCB8EAF45BC3FA016930C4A8209EA69"}, "execution_fee": "1", "sscrt": {"address": "secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3", "contract_hash": "35F5DB2BC5CD56815D10C7A567D6827BECCB8EAF45BC3FA016930C4A8209EA69"}}'
secretcli tx compute instantiate $CODE_ID "$INIT" --from a --label "BUTT Migration" -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
CONTRACT_INSTANCE_ADDRESS=secret1sshdl5qajv0q0k6shlk8m9sd4lplpn6gvf82cx
```

11. Query Config

```sh
secretcli query compute query $CONTRACT_INSTANCE_ADDRESS '{"config": {}}'
```

12. Query Orders

```sh
# Users
secretcli query compute query $CONTRACT_INSTANCE_ADDRESS '{"orders": {"address": "secret1krq6nl2qdgu66t7ghsner7sr69nz8z8v7z9t3a", "key": "testing", "page": "0", "page_size": "50"}}'
# Contract
secretcli query compute query $CONTRACT_INSTANCE_ADDRESS '{"orders": {"address": "secret1sshdl5qajv0q0k6shlk8m9sd4lplpn6gvf82cx", "key": "testing", "page": "0", "page_size": "50"}}'
```

12. Handle Msgs

```sh
# Cancel
secretcli tx compute execute $CONTRACT_INSTANCE_ADDRESS '{"cancel_order": {"position": "0"}}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
# Fill Orders
secretcli tx compute execute $CONTRACT_INSTANCE_ADDRESS '{"fill_orders": {"fill_details": [{"position": "0", "azero_transaction_hash": "asdf"}]}}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
# Register Tokens
secretcli tx compute execute $CONTRACT_INSTANCE_ADDRESS '{"register_tokens": {"tokens": [{"address": "secret18vd8fpwxzck93qlwghaj6arh4p7c5n8978vsyg", "contract_hash": "35F5DB2BC5CD56815D10C7A567D6827BECCB8EAF45BC3FA016930C4A8209EA69"}, {"address": "secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3", "contract_hash": "35F5DB2BC5CD56815D10C7A567D6827BECCB8EAF45BC3FA016930C4A8209EA69"}], "viewing_key": "testing"}}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
# Update Config
secretcli tx compute execute $CONTRACT_INSTANCE_ADDRESS '{"update_config": {"execution_fee": "1"}}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
```

13. Send SSCRT for SetExecutionFeeForOrder

```sh
# These need to be sent in a broadcast transaction but I don't know how to do it in secretcli so try it out in production.
# SetExecutionFeeForOrder
secretcli tx compute execute secret1hqrdl6wstt8qzshwc6mrumpjk9338k0lpsefm3 '{"send": { "recipient": "secret1sshdl5qajv0q0k6shlk8m9sd4lplpn6gvf82cx", "amount": "1", "msg": "eyJzZXRfZXhlY3V0aW9uX2ZlZV9mb3Jfb3JkZXIiOnt9fQ==" }}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
# CreateOrder
secretcli tx compute execute secret18vd8fpwxzck93qlwghaj6arh4p7c5n8978vsyg '{"send": { "recipient": "secret1sshdl5qajv0q0k6shlk8m9sd4lplpn6gvf82cx", "amount": "1000000", "msg": "eyJjcmVhdGVfb3JkZXIiOnsidG8iOiAiNUhpbXVTMTlNaEhYOUVnZ0Q5b1p6eDI5N3F0M1V4RWRrY2M1TldBaWFuUEFRd0hHIn19" }}' --from a -y --keyring-backend test --gas 3000000 --gas-prices=3.0uscrt
```
