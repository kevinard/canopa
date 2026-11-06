-- SPDX-License-Identifier: MIT
-- Keep the XDG frontend's per-stream grants when WirePlumber selects access.
-- Camera policy stays with WirePlumber; ScreenCast uses an empty media-role list.
if type(PermissionManager) ~= "function" then return end

local manager = PermissionManager()
manager:set_default_permissions(Perm.NONE)
manager:set_core_permissions("rwx")

-- New PipeWire versions gate a handed-off connection until its policy is ready.
-- Restore only the core handshake; the frontend's stream/factory entries survive.
manager:connect("client-properties-changed", function (_, client)
  if client:get_property("pipewire.access.portal.gated") == "true" then
    client:update_permissions { [0] = "rwx" }
  end
end)

SimpleEventHook {
  name = "client/canopa-screencast-access",
  after = "client/find-config-access",
  before = {
    "client/find-flatpak-access", "client/find-snap-access",
    "client/find-portal-access", "client/find-default-access", "client/apply-access",
  },
  interests = {
    EventInterest { Constraint { "event.type", "=", "select-access" } },
  },
  execute = function (event)
    if event:get_data("permission-manager") ~= nil
        or event:get_data("default-permissions") ~= nil then return end
    local client = event:get_subject()
    -- pipewire.access is assigned by the daemon from the public portal's PID.
    if client:get_property("pipewire.access") == "portal"
        and client:get_property("pipewire.access.portal.media_roles") == "" then
      event:set_data("permission-manager", manager)
    end
  end,
}:register()
