# Published provider command frame

`provider-commands-v1.frames` is the byte-exact serializer fixture from
[ktlsp ea8be671](https://github.com/cotrin8672/ktlsp/blob/ea8be671ed4a876a5d5514fb63daa043e953c08f/dev/external-extension/provider-commands-v1.frames).
Its 391 bytes have SHA-256
`9b9bc7a8d6dd03c18681c7b02bb638bc3284b2bfb1228cee73dbcf9abf734379`.
The transport test changes only the fixture's example command ID to a configured
MC command before dispatch and verifies that every other field survives.
