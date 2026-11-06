-- Guard the access-policy boundary; actual PipeWire behavior is checked by
-- test-screencast-flatpak.py. Run from the workspace root with `lua`.
local manager, hook
Perm = { NONE = 0 }
PermissionManager = function ()
  manager = {
    set_default_permissions = function (self, value) self.default = value end,
    set_core_permissions = function (self, value) self.core = value end,
    connect = function (self, name, callback)
      assert(name == "client-properties-changed"); self.changed = callback
    end,
  }
  return manager
end
Constraint = function (value) return value end
EventInterest = function (value) return value end
SimpleEventHook = function (value)
  hook = value
  value.register = function () end
  return value
end
dofile("packaging/portal/desktop-sharing/canopa-screencast.lua")
assert(manager.default == 0 and manager.core == "rwx")

local function event(access, roles, previous)
  local client = { props = { ["pipewire.access"] = access,
      ["pipewire.access.portal.media_roles"] = roles }, updates = {} }
  function client:get_property(key) return self.props[key] end
  function client:update_permissions(value) self.updates[#self.updates + 1] = value end
  local value = { data = previous or {}, client = client }
  function value:get_subject() return self.client end
  function value:get_data(key) return self.data[key] end
  function value:set_data(key, data) self.data[key] = data end
  hook.execute(value)
  return value, client
end
local selected = event("portal", "")
assert(selected.data["permission-manager"] == manager)
print("screen-cast-preserves-frontend-per-stream-grants")
for _, case in ipairs({ {"portal", "Camera"}, {"portal", "Camera,Microphone"},
    {"portal"}, {"unrestricted", ""}, {"flatpak", ""} }) do
  local other = event(case[1], case[2])
  assert(other.data["permission-manager"] == nil)
end
print("camera-host-direct-sandbox-and-unknown-role-policy-retained")
local previous = {}
local configured = event("portal", "", { ["permission-manager"] = previous })
assert(configured.data["permission-manager"] == previous)
configured = event("portal", "", { ["default-permissions"] = "rx" })
assert(configured.data["permission-manager"] == nil)
print("explicit-configuration-retained")
local _, client = event("portal", "")
manager.changed(manager, client)
assert(#client.updates == 0)
client.props["pipewire.access.portal.gated"] = "true"
manager.changed(manager, client)
assert(#client.updates == 1 and client.updates[1][0] == "rwx")
local count = 0
for _ in pairs(client.updates[1]) do count = count + 1 end
assert(count == 1, "Ungating must never grant fallback or unrelated-object access")
print("handoff-ungates-only-core-without-widening-stream-grants")
PermissionManager = nil
dofile("packaging/portal/desktop-sharing/canopa-screencast.lua")
print("older-policy-api-loads-without-replacing-its-access-handling")
