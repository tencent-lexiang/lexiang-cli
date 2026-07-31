using System.Diagnostics;

namespace TencentLexiang.Cli;

internal static class Program
{
    internal static int Main(string[] args)
    {
        var binaryPath = Path.Combine(AppContext.BaseDirectory, "native", "lx.exe");
        if (!File.Exists(binaryPath))
        {
            Console.Error.WriteLine(
                "The packaged native lx.exe is missing. Reinstall the tool with:\n"
                + "  dotnet tool uninstall --global TencentLexiang.Cli\n"
                + "  dotnet tool install --global TencentLexiang.Cli");
            return 1;
        }

        try
        {
            return RunNative(
                binaryPath,
                args,
                startInfo => Process.Start(startInfo)
                    ?? throw new InvalidOperationException("The native lx process did not start."));
        }
        catch (Exception error)
        {
            Console.Error.WriteLine($"Unable to start the packaged native lx.exe: {error.Message}");
            return 1;
        }
    }

    internal static int RunNative(
        string binaryPath,
        IReadOnlyList<string> arguments,
        Func<ProcessStartInfo, Process> start)
    {
        var startInfo = new ProcessStartInfo(binaryPath)
        {
            UseShellExecute = false,
            RedirectStandardInput = false,
            RedirectStandardOutput = false,
            RedirectStandardError = false,
        };
        foreach (var argument in arguments)
        {
            startInfo.ArgumentList.Add(argument);
        }

        using var process = start(startInfo);
        process.WaitForExit();
        return process.ExitCode;
    }
}
