namespace KairosEngine.Graphics
{
	public enum ColorWriteEnable : uint8
	{
		RED		= 1,
		GREEN	= 2,
		BLUE	= 4,
		ALPHA	= 8,
		ALL		= ( ( ( RED | GREEN )  | BLUE )  | ALPHA ) 
	}
}