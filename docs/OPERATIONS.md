# NStream operations

Run API load verification against a non-production environment with:

```sh
NSTREAM_API_URL=https://staging.example.com NSTREAM_API_KEY=... k6 run load/k6-api.js
```

The initial gate is fewer than 1% HTTP failures and a p95 request time below
500ms. Do not run the default scenario against production without adjusting the
arrival-rate stages to the approved capacity.

The `nstream-postgres-backup` CronJob writes PostgreSQL custom-format dumps to
the `nstream-postgres-backups` persistent volume. Replicate that volume to
off-cluster object storage and regularly restore a dump into a disposable
PostgreSQL instance; a successful dump alone is not a recovery test.
