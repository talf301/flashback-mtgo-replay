using System.Reflection;
using System.Text.Json;

// Path to the MTGO install directory — override with first CLI argument
var mtgoDir = args.Where(a => !a.StartsWith("--")).FirstOrDefault()
    ?? FindMtgoDir()
    ?? throw new Exception("MTGO install directory not found. Pass it as the first argument.");

Console.Error.WriteLine($"Loading assemblies from: {mtgoDir}");

// Hook the assembly resolver so dependencies load from the MTGO dir
AppDomain.CurrentDomain.AssemblyResolve += (_, e) =>
{
    var name = new AssemblyName(e.Name).Name;
    var path = Path.Combine(mtgoDir, name + ".dll");
    return File.Exists(path) ? Assembly.LoadFrom(path) : null;
};

// --- Mode: --reflect <TypeName> ---
// Dumps fields/properties/enum values for a named type in WotC.MtGO.Client.Model.Play.dll
if (args.Contains("--reflect"))
{
    var reflectDlls = new[] {
        "WotC.MtGO.Client.Model.Play.dll",
        "WotC.MtGO.Client.Model.Core.dll",
        "Core.dll",
        "DuelScene.dll",
        "WotC.MtGO.Client.Model.Reference.dll",
    }.Select(n => Assembly.LoadFrom(Path.Combine(mtgoDir, n))).ToArray();

    var allTypes = reflectDlls.SelectMany(a => { try { return a.GetTypes(); } catch { return []; } }).ToArray();

    var targets = args.SkipWhile(a => a != "--reflect").Skip(1).ToArray();
    if (targets.Length == 0)
    {
        targets = allTypes
            .Where(t => t.Name.Contains("Element") || t.Name == "StateElementType")
            .Select(t => t.FullName!)
            .ToArray();
    }

    foreach (var typeName in targets)
    {
        var t = allTypes.FirstOrDefault(x => x.FullName == typeName || x.Name == typeName);
        if (t == null) { Console.WriteLine($"// NOT FOUND: {typeName}"); continue; }

        Console.WriteLine($"\n=== {t.FullName} : {t.BaseType?.Name} ===");
        if (t.IsEnum)
        {
            foreach (var v in Enum.GetValues(t))
                Console.WriteLine($"  {(int)v,6}  {v}");
            continue;
        }
        var flags = BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public;
        foreach (var f in t.GetFields(flags).OrderBy(f => f.Name))
            Console.WriteLine($"  field  {f.FieldType.Name,-30} {f.Name}");
        foreach (var p in t.GetProperties(BindingFlags.Instance | BindingFlags.Public))
            Console.WriteLine($"  prop   {p.PropertyType.Name,-30} {p.Name}");
    }
    return;
}

// --- Default mode: opcode table ---
var messageDll  = Assembly.LoadFrom(Path.Combine(mtgoDir, "Message.dll"));
var mtgoMsgDll  = Assembly.LoadFrom(Path.Combine(mtgoDir, "MTGOMessage.dll"));

var csMessageType = messageDll.GetType("WotC.Common.Message.CSMessage")
    ?? throw new Exception("CSMessage not found in Message.dll");

var entries = new List<(ushort OpCode, string TypeName)>();
var errors  = new List<string>();

foreach (var t in mtgoMsgDll.GetTypes())
{
    if (t.IsAbstract || !csMessageType.IsAssignableFrom(t))
        continue;

    try
    {
        var instance = (dynamic)Activator.CreateInstance(t)!;
        ushort opCode = instance.OpCode;
        entries.Add((opCode, t.FullName ?? t.Name));
    }
    catch (Exception ex)
    {
        errors.Add($"{t.FullName}: {ex.InnerException?.Message ?? ex.Message}");
    }
}

entries.Sort((a, b) => a.OpCode.CompareTo(b.OpCode));

bool tsv = args.Contains("--tsv");

if (tsv)
{
    Console.WriteLine("opcode\ttype");
    foreach (var (opCode, typeName) in entries)
        Console.WriteLine($"{opCode}\t{typeName}");
}
else
{
    var doc = entries.Select(e => new { opcode = e.OpCode, type = e.TypeName });
    Console.WriteLine(JsonSerializer.Serialize(doc, new JsonSerializerOptions { WriteIndented = true }));
}

Console.Error.WriteLine($"\nDumped {entries.Count} message types.");
if (errors.Count > 0)
{
    Console.Error.WriteLine($"{errors.Count} types failed to instantiate:");
    foreach (var err in errors)
        Console.Error.WriteLine($"  {err}");
}

static string? FindMtgoDir()
{
    var appsDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Apps", "2.0");

    if (!Directory.Exists(appsDir)) return null;

    return Directory.EnumerateDirectories(appsDir, "*", SearchOption.AllDirectories)
        .FirstOrDefault(d => File.Exists(Path.Combine(d, "MTGOMessage.dll")));
}
