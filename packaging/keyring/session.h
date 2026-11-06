#ifndef CANOPA_KEYRING_SESSION_H
#define CANOPA_KEYRING_SESSION_H

#include <gio/gio.h>

char **user_manager_environment(GDBusConnection *bus);
int connect_canopa_display(GDBusConnection *bus, char **environment);

#endif
