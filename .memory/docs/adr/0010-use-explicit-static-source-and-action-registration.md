# Use explicit static Source and Action registration

**Rust status:** accepted

**Bun status:** superseded by ADR 0013

The retained Rust runtime will replace closed Source enums and central Source/Action dispatch matches with one explicitly populated Registry. Its separate namespaces hold typed Source and Action registrations. Built-ins remain statically linked and registered by the Rust CLI composition root.

The Bun runtime will discover Plugin Packages and resolve Plugin Descriptors as defined by ADR 0013. It will not duplicate the Rust static Registry. Both runtimes keep configuration, schema, health checks, Item Bucket identity, and runtime behavior owned by each Source or Action.