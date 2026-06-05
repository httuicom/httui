```db-postgres alias=users connection=prod limit=100 timeout=30000 display=split
SELECT id, email
FROM users
WHERE created_at > now() - interval '7 days'
```
