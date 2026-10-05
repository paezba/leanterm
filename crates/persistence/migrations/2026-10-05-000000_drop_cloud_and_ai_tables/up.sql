-- Remove persistence for the Warp Drive / teams / cloud-sync / AI features, which no longer exist.

-- Kind-specific pane tables reference pane_leaves, so drop them before deleting their leaves.
DROP TABLE IF EXISTS env_var_collection_panes;
DROP TABLE IF EXISTS workflow_panes;
DROP TABLE IF EXISTS mcp_server_panes;
DROP TABLE IF EXISTS ai_memory_panes;
DROP TABLE IF EXISTS ai_document_panes;
DROP TABLE IF EXISTS ambient_agent_panes;

-- Cloud notebook panes (no local path) can no longer be restored.
DELETE FROM notebook_panes WHERE local_path IS NULL;

DELETE FROM pane_leaves
    WHERE kind IN (
        'workflow', 'env_var_collection', 'mcp_server', 'ai_memory', 'ai_document',
        'ambient_agent', 'execution_profile_editor', 'get_started'
    )
    OR (kind = 'notebook' AND pane_node_id NOT IN (SELECT id FROM notebook_panes));

-- Delete the now-orphaned leaf pane_nodes (those with no corresponding pane_leaves row).
DELETE FROM pane_nodes
    WHERE is_leaf = 1
    AND id NOT IN (SELECT pane_node_id FROM pane_leaves);

DROP TABLE IF EXISTS object_permissions;
DROP TABLE IF EXISTS object_metadata;
DROP TABLE IF EXISTS team_members;
DROP TABLE IF EXISTS team_settings;
DROP TABLE IF EXISTS workspace_teams;
DROP TABLE IF EXISTS teams;
DROP TABLE IF EXISTS workspaces;
DROP TABLE IF EXISTS workflows;
DROP TABLE IF EXISTS notebooks;
DROP TABLE IF EXISTS folders;
DROP TABLE IF EXISTS generic_string_objects;
DROP TABLE IF EXISTS object_actions;
DROP TABLE IF EXISTS cloud_objects_refreshes;
DROP TABLE IF EXISTS server_experiments;
DROP TABLE IF EXISTS user_profiles;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS current_user_information;
DROP TABLE IF EXISTS agent_tasks;
DROP TABLE IF EXISTS agent_conversations;
DROP TABLE IF EXISTS ai_queries;
DROP TABLE IF EXISTS active_mcp_servers;
DROP TABLE IF EXISTS mcp_environment_variables;
DROP TABLE IF EXISTS mcp_server_installations;
DROP TABLE IF EXISTS project_rules;
DROP TABLE IF EXISTS projects;
