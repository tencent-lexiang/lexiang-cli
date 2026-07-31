using System.Diagnostics;
using Xunit;

namespace TencentLexiang.Cli.Tests;

public sealed class LauncherTests
{
    [Fact]
    public void RunNativeForwardsArgumentsWithoutShellOrRedirects()
    {
        ProcessStartInfo? captured = null;
        var arguments = new[] { "tool", "call", "--data-raw", "{\"value\":\"a b\"}" };

        var exitCode = Program.RunNative(
            "C:\\packaged path\\lx.exe",
            arguments,
            startInfo =>
            {
                captured = startInfo;
                return StartExitProcess(23);
            });

        Assert.Equal(23, exitCode);
        Assert.NotNull(captured);
        Assert.Equal("C:\\packaged path\\lx.exe", captured.FileName);
        Assert.Equal(arguments, captured.ArgumentList);
        Assert.False(captured.UseShellExecute);
        Assert.False(captured.RedirectStandardInput);
        Assert.False(captured.RedirectStandardOutput);
        Assert.False(captured.RedirectStandardError);
    }

    [Fact]
    public void MainExplainsHowToRepairAMissingNativeBinary()
    {
        var originalError = Console.Error;
        using var capturedError = new StringWriter();
        Console.SetError(capturedError);
        try
        {
            var exitCode = Program.Main(["version"]);

            Assert.NotEqual(0, exitCode);
            Assert.Contains("dotnet tool uninstall --global TencentLexiang.Cli", capturedError.ToString());
            Assert.Contains("dotnet tool install --global TencentLexiang.Cli", capturedError.ToString());
        }
        finally
        {
            Console.SetError(originalError);
        }
    }

    private static Process StartExitProcess(int exitCode)
    {
        ProcessStartInfo startInfo;
        if (OperatingSystem.IsWindows())
        {
            startInfo = new ProcessStartInfo("cmd.exe");
            startInfo.ArgumentList.Add("/d");
            startInfo.ArgumentList.Add("/c");
            startInfo.ArgumentList.Add($"exit {exitCode}");
        }
        else
        {
            startInfo = new ProcessStartInfo("/bin/sh");
            startInfo.ArgumentList.Add("-c");
            startInfo.ArgumentList.Add($"exit {exitCode}");
        }
        startInfo.UseShellExecute = false;
        return Process.Start(startInfo) ?? throw new InvalidOperationException("Failed to start test process");
    }
}
