-- Make status mandatory and backfill
-- Write the entire migration as a transaction
BEGIN;
  -- Backfill
  UPDATE subscriptions
    SET status = 'confirmed'
    WHERE status IS NULL;
  -- Make mandatory
  ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
COMMIT;
