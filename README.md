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

# Get the contract's id
secretcli query compute list-code
```
