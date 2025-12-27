namespace KairosEngine.Graphics
{
	public enum RootParameterType : uint32
	{
		DESCRIPTOR_TABLE	= 0,
		_32BIT_CONSTANTS	= ( DESCRIPTOR_TABLE + 1 ) ,
		CBV	= ( _32BIT_CONSTANTS + 1 ) ,
		SRV	= ( CBV + 1 ) ,
		DUAV	= ( SRV + 1 ) 
	}
}