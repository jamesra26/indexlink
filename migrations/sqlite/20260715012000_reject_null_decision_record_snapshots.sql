-- Reject JSON null for required decision-record snapshots.
--
-- SQLite json_valid('null') is true, while the domain treats every required
-- snapshot as present and non-null. Existing rows remain readable only when
-- they satisfy the adapter's stricter read validation; these triggers stop
-- future direct SQL writes from introducing invalid snapshots.

CREATE TRIGGER decision_records_required_snapshots_insert_check
BEFORE INSERT ON decision_records
FOR EACH ROW
WHEN json_type(NEW.execution_snapshot) = 'null'
    OR json_type(NEW.fundamental_snapshot) = 'null'
    OR json_type(NEW.trend_snapshot) = 'null'
    OR json_type(NEW.decision_snapshot) = 'null'
BEGIN
    SELECT RAISE(ABORT, 'required decision record snapshots must not be JSON null');
END;

CREATE TRIGGER decision_records_required_snapshots_update_check
BEFORE UPDATE OF execution_snapshot, fundamental_snapshot, trend_snapshot, decision_snapshot
ON decision_records
FOR EACH ROW
WHEN json_type(NEW.execution_snapshot) = 'null'
    OR json_type(NEW.fundamental_snapshot) = 'null'
    OR json_type(NEW.trend_snapshot) = 'null'
    OR json_type(NEW.decision_snapshot) = 'null'
BEGIN
    SELECT RAISE(ABORT, 'required decision record snapshots must not be JSON null');
END;
