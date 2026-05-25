# pgone-cli

Unified command-line entrypoint for PGone.

## Usage

Launch the GUI, which is the default command:

```bash
cargo run -p pgone-cli --
```

The explicit GUI command is equivalent:

```bash
cargo run -p pgone-cli -- gui
```

Run the MCP server through the same binary:

```bash
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio
```

Set the log level for any command with `--log-level`:

```bash
cargo run -p pgone-cli -- --log-level debug gui
cargo run -p pgone-cli -- mcp-server --dbconfig-id default --protocol stdio --log-level warn
```

The package name is `pgone-cli`, and the installed binary name is `pgone`.
Existing service-specific binaries remain available for compatibility, including `pgone-mcp-server` from the `pgone-mcp` crate.
