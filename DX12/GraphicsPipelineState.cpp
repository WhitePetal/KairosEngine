#include "GraphicsPipelineState.h"

void KAIROS_API GraphicsPipelineState_Dispose(ID3D12PipelineState* _this)
{
    _this->Release();
}
