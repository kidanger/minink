## Compilation

```
cd minink-agent/
sqlx database reset --database-url sqlite://logs.db
cargo sqlx prepare --database-url sqlite://$PWD/logs.db
```
