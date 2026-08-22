CREATE EXTENSION IF NOT EXISTS pgcrypto;
ALTER TABLE webhook_subscriptions ADD COLUMN signing_secret_ciphertext BYTEA;
ALTER TABLE webhook_subscriptions ALTER COLUMN signing_secret DROP NOT NULL;
-- Legacy secrets cannot be safely encrypted without a deployment-specific key.
-- Disable their delivery by removing plaintext; subscriptions must be re-created.
UPDATE webhook_subscriptions SET signing_secret = NULL WHERE signing_secret IS NOT NULL;
