namespace KairosEngine.Graphics
{
	public enum HeapType : uint32
	{
		DEFAULT		= 1,
		UPLOAD		= 2,
		READBACK	= 3,
		CUSTOM		= 4,
		GPU_UPLOAD	= 5
	}
}