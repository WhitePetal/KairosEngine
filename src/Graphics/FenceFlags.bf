namespace KairosEngine.Graphics
{
	public enum FenceFlags : uint
	{
		NONE					= 0,
		SHARED					= 0x1,
		SHARED_CROSS_ADAPTER	= 0x2,
		NON_MONITORED			= 0x4
	}
}