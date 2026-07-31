using System.Diagnostics;

var binary = Path.Combine(AppContext.BaseDirectory, "native", "lx.exe");
if (!File.Exists(binary))
{
    Console.Error.WriteLine("Packaged lx.exe is missing; reinstall TencentLexiang.Cli.");
    return 1;
}

var startInfo = new ProcessStartInfo(binary)
{
    UseShellExecute = false
};
foreach (var argument in args)
{
    startInfo.ArgumentList.Add(argument);
}

using var process = Process.Start(startInfo);
if (process is null)
{
    Console.Error.WriteLine("Failed to start packaged lx.exe.");
    return 1;
}

process.WaitForExit();
return process.ExitCode;

