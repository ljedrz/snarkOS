# snarkos-metrics

[![Crates.io](https://img.shields.io/crates/v/snarkos-metrics.svg?color=neon)](https://crates.io/crates/snarkos-metrics)
[![Authors](https://img.shields.io/badge/authors-Aleo-orange.svg)](../AUTHORS)
[![License](https://img.shields.io/badge/License-GPLv3-blue.svg)](./LICENSE.md)

## Development

To start a local instance of Prometheus for development purposes, run:
```
cd prometheus/{YOUR_OS}
docker build -t prometheus .
docker run -p 9090:9090 --network=host prometheus
```

Then, to start `snarkos-metrics` on its own, run:
```
cargo run
```

To confirm the `snarkos-metrics` instance is up, navigate to [https://localhost:8080/metrics](https://localhost:8080/metrics).

To confirm the Prometheus instance is up, navigate to [https://localhost:9090/graph](https://localhost:9090/graph).

## Security

To enable access to metrics, open ports `8080` and `9090` on your development or production machine.
