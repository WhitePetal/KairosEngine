namespace KairosEngine.Graphics
{
	public enum StaticBorderColor : uint32
	{
		TRANSPARENT_BLACK	= 0,
		OPAQUE_BLACK		= ( TRANSPARENT_BLACK + 1 ) ,
		OPAQUE_WHITE		= ( OPAQUE_BLACK + 1 ) ,
		OPAQUE_BLACK_UINT	= ( OPAQUE_WHITE + 1 ) ,
		OPAQUE_WHITE_UINT	= ( OPAQUE_BLACK_UINT + 1 ) 
	}
}