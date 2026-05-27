--main_.lua
--[[
This library was made by Radiant.
This library IS NOT FINIHED.
This is just a preview of the library.
]]
local m = Instance.new("ModuleScript")
m.Source = [[
local Boolq
local Core
local EnumCreator
local EventCreator
local find_table
local HeatObject
local Math
local newheatobjfunc
do
--src/boolq.lua
Boolq = {
	["false"] = setmetatable({},{
		__eq = function(t,o)
			if o == false then return true end
			return rawequal(t,o)
		end,
		__tostring= function(t)
			return "boolq.false"
		end,
	}),
	["true"] = setmetatable({},{
		__eq = function(t,o)
			if o == true then return true end
			return rawequal(t,o)
		end,
		__tostring= function(t)
			return "boolq.true"
		end,
	}),
	["unknown"] = setmetatable({},{
		__tostring= function(t)
			return "boolq.unknown"
		end,
	})
}
--src/dmr.lua
Core = {}
function Core:classprotect()
    if self.heatsystem == nil then error("Cannot execute instance method on class!") end
end
function Core:new(obj)
    if not Core.CoreStates then
        Core.CoreStates = EnumCreator.newEnum("CoreStates",{
            DISABLED=true,
            SHUTDOWN=true,
            OFFLINE=true,
            STARTING=true,
            ONLINE=true,
            MELTDOWN=true,
            SHUTDOWNMELTDOWN=true,
            MELTDOWNP2=true,
            SHUTDOWNMELTDOWNP2=true,
            MELTDOWNP3=true
        })
        Core.ShutdownReasons = EnumCreator.newEnum("ShutdownReasons", {
            Admin=true,
            Stall=true,
            Emergency=true,
            Intended=true
        })
        Core.offlineStates = {Core.CoreStates.DISABLED,Core.CoreStates.OFFLINE}
    end
    local object = obj or setmetatable({},{__index=self})

    object.heatsystem = HeatObject.newSystem()
    object.outercasing = HeatObject.newObject()
    object.intermediary_isolation = HeatObject.newObject()
    object.internalcasing = HeatObject.newObject()
    object.combustionchamber = HeatObject.newObject()
    object.combustionvalve = HeatObject.newObject()
    object.fuelinjector = HeatObject.newObject()
    object.internalcoolingloop = HeatObject.newObject()
    object.externalcoolingloop = HeatObject.newObject()
    object.heatsystem.add(object.outercasing)
    object.heatsystem.add(object.intermediary_isolation)
    object.heatsystem.add(object.internalcasing)
    object.heatsystem.add(object.combustionchamber)
    object.heatsystem.add(object.combustionvalve)
    object.heatsystem.add(object.fuelinjector)
    object.heatsystem.add(object.internalcoolingloop)
    object.heatsystem.add(object.externalcoolingloop)
    object.outercasing.conductivity = 0.3
    object.intermediary_isolation.conductivity = 0.2
    object.internalcasing.conductivity = 0.8
    object.combustionchamber.conductivity = 1
    object.combustionvalve.conductivity = 0.02
    object.fuelinjector.conductivity = 0.017
    object.externalcoolingloop.conductivity = 0.9
    object.internalcoolingloop.conductivity = 0.95
    object.outercasing.mass = 170
    object.intermediary_isolation.mass=0.7
    object.internalcasing.mass = 15476
    object.combustionchamber.mass=1572
    object.fuelinjector.mass = 17
    object.internalcoolingloop.mass = 3571
    object.externalcoolingloop.mass = 4612

    object.outercasing.addn(object.intermediary_isolation)
    object.intermediary_isolation.addn(object.internalcasing)
    object.internalcasing.addn(object.combustionchamber)
    object.fuelinjector.addn(object.combustionchamber)
    object.fuelinjector.addn(object.internalcasing)
    object.combustionchamber.addn(object.combustionvalve)
    object.externalcoolingloop.addn(object.internalcasing)
    object.internalcoolingloop.addn(object.combustionchamber)

    object.combustionvalve.isweld = false

    --consts
    object.dangerfactor = 2.5
    object.onShutdown = EventCreator.newPublicEvent() -- onEvent: shutdown reason
    object.onStartup = EventCreator.newPublicEvent()  -- onEvent: nil (no args)
    object.onUpdate = EventCreator.newPublicEvent()   -- onEvent: nil (no args)
    --------
    
    object.lastresults = {}
    object.lastresults.heatpower = 0
    object.lastresults.heatOffset = 0

    object.state = Core.CoreStates.OFFLINE
    object.valvestate = 0
    object.valvetarget = 0
    object.internalcoolingrate = 0 --energy/s
    object.externalcoolingrate = 0

    function object.update()
        if object.combustionvalve.temperature > 3973.15 then object.combustionvalve.isweld = true end
        if not Core:isOffline() then
            local hp,icp,ecp = self:calcPower()
            object.combustionchamber:changeEnergy(hp)
            object.internalcoolingloop:changeEnergy(-icp)
            object.externalcoolingloop:changeEnergy(-ecp)
        end
        object.heatsystem.updatecycle()
    end

    return object
end
function Core:advance(...)
    Core.classprotect(self)
    return setmetatable({
        [Core.CoreStates.DISABLED]=function(...)
            return Core.CoreStates.DISABLED
        end,
        [Core.CoreStates.SHUTDOWN]=function(...)
            self.onShutdown:Fire(table.pack(...)[1] or Core.ShutdownReasons.Intended)
            return Core.CoreStates.OFFLINE
        end,
        [Core.CoreStates.OFFLINE]=function(...)
            return Core.CoreStates.STARTING
        end,
        [Core.CoreStates.STARTING]=function(...)
            self.onStartup:Fire()
            return Core.CoreStates.ONLINE
        end,
        [Core.CoreStates.ONLINE]=function(...)
            return Core.CoreStates.MELTDOWN
        end,
        [Core.CoreStates.MELTDOWN]=function(...)
            if table.pack(...)[1] then
                return Core.CoreStates.SHUTDOWNMELTDOWN
            else
                return Core.CoreStates.MELTDOWNP2
            end
        end,
        [Core.CoreStates.MELTDOWNP2]=function(...)
            if table.pack(...)[1] then
                return Core.CoreStates.SHUTDOWNMELTDOWNP2
            else
                return Core.CoreStates.MELTDOWNP3
            end
        end,
        [Core.CoreStates.SHUTDOWNMELTDOWN]=function(...)
            if table.pack(...)[1] then
                return Core.CoreStates.OFFLINE
            else
                return Core.CoreStates.MELTDOWNP2
            end
        end,
        [Core.CoreStates.SHUTDOWNMELTDOWNP2]=function(...)
            if table.pack(...)[1] then
                return Core.CoreStates.OFFLINE
            else
                return Core.CoreStates.MELTDOWNP3
            end
        end,
    },{__index=function(t,k) error("Cannot find state "..k) end})[self.state](...)
end
function Core:getTemp()
    Core.classprotect(self)
    return self.internalcasing.temperature
end
function Core:isOffline()
    Core.classprotect(self)
    return find_table(Core.offlineStates,self.state) ~= nil or self.lastresults.heatpower < 10
end
function Core:CombustionValveError()
    Core.classprotect(self)
    return self.combustionvalve.isweld
end
function Core:CombsutionValveState()
    Core.classprotect(self)
    return self.valvestate
end
function Core:calcPower()
    Core.classprotect(self)
    local heatpower = 0
    self.lastresults.heatOffset = (self.dangerfactor ^ 1.5) ^ (self.combustionchamber.temperature / 1000)
    heatpower = self.lastresults.heatOffset
              * self:calcValvePower()
              * self:calcfuelinjector() * 50
    if self.state == Core.CoreStates.STARTING then
        heatpower = heatpower + self:calcfuelinjector() * 50 * 4
    end
    self.lastresults.heatpower = heatpower
    return heatpower,self.internalcoolingrate / 60,self.externalcoolingrate / 60
end
function Core:calcfuelinjector()
    Core.classprotect(self)
    return math.log(self.fuelinjector.temperature/50 + 1,10)*4
end
function Core:calcValvePower()
    Core.classprotect(self)
    return Math.sigmoid((self.valvestate - 0.5) * 8)
end
function Core:calcSustainability()
    Core.classprotect(self)
    local h = 60 / 1.5 / 4 * self.lastresults.heatpower
    h = -math.log((h * -4) / 10^3 + 4.5,10) + .65
    if h > 1 or Math.isnan(h) then
        h = 1
    elseif h < 0 then
        h = 0
    end
    return h
end
function Core:calcValveMoveBounds()
    Core.classprotect(self)
    if self.combustionvalve.isweld then
        return self.valvestate,self.valvestate
    end
    local speed = 1/60/3;
    local max = self.valvestate+speed;
    local min = self.valvestate-speed;
    if max > 1 then max = 1 end
    if min < 0 then min = 0 end
    return max,min
end
--src/enumcreator.lua
EnumCreator = {}
local enums = setmetatable({},{__mode="k"})
local enum_mt = {__tostring = function(t)
	return enums[t]
end,}
function EnumCreator.newenum_t (name)
	local t = setmetatable({},enum_mt)
	enums[t] = name
	return t
end
function EnumCreator.newEnum(name, enum)
	local root = EnumCreator.newenum_t(name)
	for k,v in pairs(enum) do
        if type(v) == "boolean" then
            local t = EnumCreator.newenum_t(name.."."..k)
            root[k] = t
        else
            root[k] = EnumCreator.newEnum(name.."."..k,v)
        end
    end
	return root
end

--src/eventcreator.lua
EventCreator = {}
function EventCreator.newProtectedEvent()
	local events = {}
	local ev = {}
	function ev:Connect(func)
		table.insert(events,func)
		local eh = {}
		function eh:Disconnect()
			table.remove(events,find_table(events,func))
		end
		return eh
	end
	function ev:Wait()
		local thread = coroutine.running()
		local eh
		eh = ev:Connect(function (...)
			eh:Disconnect()
			coroutine.resume(thread,...)
		end)
		return coroutine.yield()
	end
	return ev, function(...)
		local t = table.pack(...)
		for _,f in ipairs(events) do
            coroutine.wrap(f)(table.unpack(t))
        end
	end
end
function EventCreator.newPublicEvent()
	local events = {}
	local ev = {}
	function ev:Connect(func)
		table.insert(events,func)
		local eh = {}
		function eh:Disconnect()
			table.remove(events,find_table(events,func))
		end
		return eh
	end
	function ev:Wait()
		local thread = coroutine.running()
		local eh
		eh = ev:Connect(function (...)
			eh:Disconnect()
			coroutine.resume(thread,...)
		end)
		return coroutine.yield()
	end
	function ev:Fire(...)
		local t = table.pack(...)
		for _,f in ipairs(events) do
            coroutine.wrap(f)(table.unpack(t))
        end
	end
	return ev
end

--src/find_table.lua
function find_table(tbl, v)
    for k,_v in pairs(tbl) do if _v == v then return k end end
end
--src/heatobjects.lua
HeatObject = {}
function HeatObject.newObject()
	return setmetatable({
		neighbours={},
		conductivity=1,
		temperature=287.15,
		_temp=287.15,
		mass=1,
	},HeatObject)
end
function HeatObject.update(h)
	local x = 0
	for _,v in ipairs(h.neighbours) do
		x = newheatobjfunc(
			h.temperature,
			v.temperature,
			h.conductivity,
			v.conductivity,
			h.mass,
			v.mass
		) + x
	end
	h._temp = x / #(h.neighbours)
end
function HeatObject.apply(h)
	h.temperature = h._temp
end
function HeatObject.newSystem()
	local h = {
		objects={}
	}
	function h.add(o)
		table.insert(h.objects,o)
	end
	function h.update()
		for _,o in ipairs(h.objects) do
			HeatObject.update(o)
		end
	end
	function h.apply()
		for _,o in ipairs(h.objects) do
			HeatObject.apply(o)
		end
	end
	function h.updatecycle()
		h.update()
		h.apply()
	end
	return h
end
function HeatObject:getEnergy()
	return self.temperature * self.mass
end
function HeatObject:setEnergy(val)
	self.temperature = (self.mass / val) ^ -1
end
function HeatObject:changeEnergy(val)
	self:setEnergy(self:getEnergy() + val)
end
function HeatObject.addn(self, o) 
	table.insert(self.neighbours, o)
	table.insert(o.neighbours, self)
end
--src/math.lua
Math = {}
Math.inf = 1/0
Math.nan = -(0/0)
function Math.sigmoid(x)
    return 1/(1+math.exp(-x))
end
function Math.isnan(x)
    return x ~= x
end
function Math.round(val, digits)
	digits = digits or 0
	return string.format("%."..digits.."f",val)
end
function Math.scale(num,
    from_min, from_max,
    to_min,   to_max   )
	return (((num - from_min) * (to_max - to_min)) / (from_max - from_min)) + to_min
end
--src/newheatobjfunc.lua
function newheatobjfunc(st,ot,sc,oc,sm,om)
    local oc = math.min(1,om/sm)*sc*oc
    return ot*oc + st*(1-oc)
end
--main.lua
local module = {}
module.boolq = Boolq
module.dmr = Core
module.heatObj = HeatObject
return module
end
]]
m.Parent = workspace
local f = Instance.new("Folder")
f.Parent = game:GetService("ReplicatedStorage")
f.Name = "Modules"
m.Parent = f
print(m)
task.defer(function()
    m = nil
    game:DebugPrintTree()
    task.defer(function()
        game:DebugPrintTree()
        game:Shutdown()
    end)
end)