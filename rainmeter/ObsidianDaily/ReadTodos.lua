local CHARS_PER_LINE = 36   -- approx for Segoe UI 10pt in ~245px width
local PX_PER_LINE    = 22

function Initialize()
end

function Update()
    local offset = tonumber(SKIN:GetVariable("DayOffset", "0")) or 0
    local now    = os.time() + offset * 86400
    local date   = os.date("*t", now)

    local year  = date.year
    local month = string.format("%02d", date.month)
    local day   = string.format("%02d", date.day)

    local datestr     = os.date("%A, %d.%m.%Y", now)
    local journalPath = string.format("Personal/Journal/%d/%s/%d-%s-%s", year, month, year, month, day)
    SKIN:Bang('!SetVariable', 'DateString',  datestr)
    SKIN:Bang('!SetVariable', 'JournalPath', journalPath)

    local home  = os.getenv("USERPROFILE") or "C:\\Users\\jherng"
    local vault = home .. "\\Documents\\Obsidian Vault"
    local path  = string.format(
        "%s\\Personal\\Journal\\%d\\%s\\%d-%s-%s.md",
        vault, year, month, year, month, day
    )

    local file = io.open(path, "r")
    if not file then
        SKIN:Bang('!SetVariable', 'TodosHeight', '40')
        return "  No journal file."
    end

    local pending = {}
    local done    = {}

    for line in file:lines() do
        local is_done    = line:match("^%- %[x%]")
        local is_pending = line:match("^%- %[ %]")

        if is_done or is_pending then
            local clean = line
                :gsub("^%- %[[x ]%]%s*", "")
                :gsub("%b[]", "")
                :gsub("`[^`]*`", "")
                :gsub("%b()", "")
                :gsub("%^%S+", "")
                :gsub("%d%d%d%d%-%d%d%-%d%d", "")
                :gsub("[\128-\255]+", "")
                :gsub("%s+", " ")
                :gsub("^%s*(.-)%s*$", "%1")

            if #clean > 0 then
                if is_done then
                    table.insert(done,    "[x]  " .. clean)
                else
                    table.insert(pending, "[ ]  " .. clean)
                end
            end
        end
    end
    file:close()

    local lines = {}
    for _, v in ipairs(pending) do table.insert(lines, v) end
    if #done > 0 then
        if #pending > 0 then table.insert(lines, "") end
        for _, v in ipairs(done) do table.insert(lines, v) end
    end

    -- estimate widget height by counting wrapped visual lines
    local visualLines = 0
    for _, l in ipairs(lines) do
        if #l == 0 then
            visualLines = visualLines + 1
        else
            visualLines = visualLines + math.max(1, math.ceil(#l / CHARS_PER_LINE))
        end
    end
    local heightPx = math.max(40, visualLines * PX_PER_LINE + 12)
    SKIN:Bang('!SetVariable', 'TodosHeight', tostring(heightPx))

    if #lines == 0 then
        SKIN:Bang('!SetVariable', 'TodosHeight', '40')
        return "  All clear!"
    end

    return table.concat(lines, "\n")
end
