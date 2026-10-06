using System.Runtime.CompilerServices;
using System.Text;

// Program.cs reports on the console and through its exit code, and an app launched in the iOS
// simulator hands neither back to the shell that launched it. So everything it writes also
// goes to a file in the app's temporary directory, which is inside the app's container in the
// simulator, where CI reads it, and the user's temporary directory on Mac Catalyst. A module
// initializer, because Program.cs is top-level statements shared with the Native AOT smoke
// test and has nowhere to put this.
internal static class SmokeLog
{
	internal const string FileName = "hyperuuid-smoke.log";

	[ModuleInitializer]
	internal static void Start()
	{
		var file = new StreamWriter(Path.Combine(Path.GetTempPath(), FileName), append: false) { AutoFlush = true };
		Console.SetOut(new Tee(Console.Out, file));
	}

	private sealed class Tee(TextWriter console, TextWriter file) : TextWriter
	{
		public override Encoding Encoding => console.Encoding;

		public override void Write(char value)
		{
			console.Write(value);
			file.Write(value);
		}

		public override void Write(string? value)
		{
			console.Write(value);
			file.Write(value);
		}

		public override void WriteLine(string? value)
		{
			console.WriteLine(value);
			file.WriteLine(value);
		}
	}
}
