using System.Text;
using Android.App;
using Android.OS;
using Android.Util;

namespace HyperUuid.AndroidSmokeTest;

// An Android app is started by its launcher Activity, never by a Main, so this one runs the
// Native AOT smoke test's SmokeTest.Run() itself. Everything it writes also goes to logcat
// under the tag HyperUuidSmoke, which is what CI reads, along with the exit line written
// last. The Java name is fixed so CI can start the Activity by name instead of by the
// generated crc64 one.
[Activity(Name = "io.github.skunkwerkx.hyperuuid.androidsmoketest.SmokeActivity", Label = "HyperUuid smoke test",
	MainLauncher = true, Exported = true)]
public sealed class SmokeActivity : Activity
{
	internal const string Tag = "HyperUuidSmoke";

	protected override void OnCreate(Bundle? savedInstanceState)
	{
		base.OnCreate(savedInstanceState);
		Console.SetOut(new LogcatWriter());
		int code;
		try
		{
			code = AotSmokeTest.SmokeTest.Run();
		}
		catch (Exception exception)
		{
			Log.Error(Tag, exception.ToString());
			code = 2;
		}
		Log.Info(Tag, $"exit code {code}");
		FinishAndRemoveTask();
	}

	// One logcat line per Console line.
	private sealed class LogcatWriter : TextWriter
	{
		private readonly StringBuilder line = new();

		public override Encoding Encoding => Encoding.UTF8;

		public override void Write(char value)
		{
			if (value == '\n')
			{
				Log.Info(Tag, line.ToString());
				line.Clear();
			}
			else if (value != '\r')
			{
				line.Append(value);
			}
		}

		public override void WriteLine(string? value)
		{
			line.Append(value);
			Write('\n');
		}
	}
}
