use super::{app_database_file_path, database_file_path_for_scope};
use crate::persistence::PersistenceScope;

#[test]
fn app_scope_database_path_matches_app_database_path() {
    assert_eq!(
        database_file_path_for_scope(&PersistenceScope::App),
        app_database_file_path()
    );
}
