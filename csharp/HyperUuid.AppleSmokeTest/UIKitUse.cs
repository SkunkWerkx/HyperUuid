using System.Runtime.CompilerServices;
using UIKit;

// What every real iOS or Mac Catalyst app does and this one otherwise wouldn't: use UIKit. The
// smoke test's code is the Native AOT smoke test's and touches no Apple API, so without this the
// trimmer drops the platform assembly (Microsoft.iOS, Microsoft.MacCatalyst) that the runtime calls
// into at startup, and on .NET 11's default registrar, trimmable-static, the app aborts before
// Main (xamarin_bridge_call_runtime_initialize: failed to create delegate). The SDK also links
// only the frameworks the app is seen to use, and the runtime's own libxamarin references this
// very symbol, so the same line keeps UIKit in the native link. A module initializer, for the
// reason SmokeLog.cs gives.
internal static class UIKitUse
{
	[ModuleInitializer]
	internal static void Touch() => GC.KeepAlive(UIApplication.DidReceiveMemoryWarningNotification);
}
