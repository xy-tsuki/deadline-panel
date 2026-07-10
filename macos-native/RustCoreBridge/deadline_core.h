#ifndef DEADLINE_CORE_H
#define DEADLINE_CORE_H

#include <stdbool.h>

char *deadline_core_version_json(void);
char *deadline_initialize_json(const char *database_path);
char *deadline_migrate_legacy_database_json(const char *legacy_database_path);
char *deadline_import_tasks_json(const char *input);
char *deadline_list_json(void);
char *deadline_create_json(const char *input);
char *deadline_update_json(const char *input);
bool deadline_delete(const char *id);
char *deadline_complete_json(const char *id);
char *deadline_restore_json(const char *id);
char *deadline_toggle_current_json(const char *id);
char *deadline_parse_quick_add_json(const char *input);
void deadline_free_string(char *ptr);

#endif
