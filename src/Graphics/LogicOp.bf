namespace KairosEngine.Graphics
{
	public enum LogicOp : uint32
	{
		CLEAR			= 0,
		SET				= ( CLEAR + 1 ) ,
		COPY			= ( SET + 1 ) ,
		COPY_INVERTED	= ( COPY + 1 ) ,
		NOOP			= ( COPY_INVERTED + 1 ) ,
		INVERT			= ( NOOP + 1 ) ,
		AND				= ( INVERT + 1 ) ,
		NAND			= ( AND + 1 ) ,
		OR				= ( NAND + 1 ) ,
		NOR				= ( OR + 1 ) ,
		XOR				= ( NOR + 1 ) ,
		EQUIV			= ( XOR + 1 ) ,
		AND_REVERSE		= ( EQUIV + 1 ) ,
		AND_INVERTED	= ( AND_REVERSE + 1 ) ,
		OR_REVERSE		= ( AND_INVERTED + 1 ) ,
		OR_INVERTED		= ( OR_REVERSE + 1 ) 
	}
}