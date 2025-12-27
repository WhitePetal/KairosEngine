#include "GraphicsDevice.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDevice_Dispose(ID3D12Device* _this)
{
	_this->Release();
}

int KAIROS_API GraphicsDevice_CreateCommandQueue(ID3D12Device* _this, ID3D12CommandQueue** ppCommandQueue, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask)
{
	HRESULT hr;
	D3D12_COMMAND_QUEUE_DESC desc = { type, priority, flags, nodeMask };
	ID3D12CommandQueue* pQueue;
	hr = _this->CreateCommandQueue(&desc, IID_PPV_ARGS(&pQueue));
	*ppCommandQueue = pQueue;
	return hr;
}

int KAIROS_API GraphicsDevice_CreateDescriptorHeap(ID3D12Device* _this, ID3D12DescriptorHeap** ppDescriptorHeap, int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags)
{
	D3D12_DESCRIPTOR_HEAP_DESC desc = { type, count, flags };
	ID3D12DescriptorHeap* pDescriptorHeap;
	HRESULT hr = _this->CreateDescriptorHeap(&desc, IID_PPV_ARGS(&pDescriptorHeap));
	*ppDescriptorHeap = pDescriptorHeap;
	return hr;
}

UINT KAIROS_API GraphicsDevice_GetDescriptorHandleIncrementSize(ID3D12Device* _this, D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType)
{
	return _this->GetDescriptorHandleIncrementSize(descriptorHeapType);
}

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHandle(ID3D12Device* _this, ID3D12Resource* pRenderTarget, D3D12_CPU_DESCRIPTOR_HANDLE handle)
{
	_this->CreateRenderTargetView(pRenderTarget, nullptr, handle);
}

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHeapIndex(ID3D12Device* _this, ID3D12Resource* pRenderTarget, ID3D12DescriptorHeap* pDescriptorHeap, int index)
{
	ID3D12DescriptorHeap* ipDescriptorHeap = pDescriptorHeap;
	UINT descriptorSize = _this->GetDescriptorHandleIncrementSize(ipDescriptorHeap->GetDesc().Type);
	CD3DX12_CPU_DESCRIPTOR_HANDLE handle{ ipDescriptorHeap->GetCPUDescriptorHandleForHeapStart() };
	handle.Offset(index, descriptorSize);
	_this->CreateRenderTargetView(pRenderTarget, nullptr, handle);
}

int KAIROS_API GraphicsDevice_CreateCommandAllocator(ID3D12Device* _this, ID3D12CommandAllocator** ppCommandAllocator, D3D12_COMMAND_LIST_TYPE type)
{
	ID3D12CommandAllocator* pCommandAllocator;
	HRESULT hr = _this->CreateCommandAllocator(type, IID_PPV_ARGS(&pCommandAllocator));
	*ppCommandAllocator = pCommandAllocator;
	return hr;
}

int KAIROS_API GraphicsDevice_CreateCommandList(ID3D12Device* _this, ID3D12CommandList** ppCommandList, ID3D12CommandAllocator* pCommandAllocator, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask)
{
	ID3D12GraphicsCommandList* pCommandList;
	HRESULT hr = _this->CreateCommandList(nodeMask, type, pCommandAllocator, NULL, IID_PPV_ARGS(&pCommandList));
	*ppCommandList = pCommandList;
	return hr;
}

int KAIROS_API GraphicsDevice_CreateFence(ID3D12Device* _this, ID3D12Fence** ppFence, UINT64 initialValue, D3D12_FENCE_FLAGS flags)
{
	ID3D12Fence* pFence;
	HRESULT hr = _this->CreateFence(initialValue, flags, IID_PPV_ARGS(&pFence));
	*ppFence = pFence;
	return hr;
}

int KAIROS_API GraphicsDevice_CreateEmptyRootSignature(ID3D12Device* _this, ID3D12RootSignature** ppRootSignature, D3D12_ROOT_SIGNATURE_FLAGS flags)
{
	CD3DX12_ROOT_SIGNATURE_DESC rootSignatureDesc;
	rootSignatureDesc.Init(0, nullptr, 0, nullptr, flags);

	HRESULT hr;
	ID3DBlob* signature;
	hr = D3D12SerializeRootSignature(&rootSignatureDesc, D3D_ROOT_SIGNATURE_VERSION_1, &signature, nullptr);
	if (FAILED(hr))
		return hr;

	ID3D12RootSignature* pRootSignature;
	hr = _this->CreateRootSignature(0, signature->GetBufferPointer(), signature->GetBufferSize(), IID_PPV_ARGS(&pRootSignature));
	SAFE_RELEASE(signature);
	*ppRootSignature = pRootSignature;
	return hr;
}

int KAIROS_API GraphicsDevice_CreateRootSignature(ID3D12Device* _this, ID3D12RootSignature** ppRootSignature, D3D12_ROOT_SIGNATURE_DESC* pDesc)
{
	HRESULT hr;
	ID3DBlob* signature;
	hr = D3D12SerializeRootSignature(pDesc, D3D_ROOT_SIGNATURE_VERSION_1, &signature, nullptr);
	if (FAILED(hr))
		return hr;
	ID3D12RootSignature* pRootSignature;
	hr = _this->CreateRootSignature(0, signature->GetBufferPointer(), signature->GetBufferSize(), IID_PPV_ARGS(&pRootSignature));
	SAFE_RELEASE(signature);
	*ppRootSignature = pRootSignature;
	return hr;
}

int KAIROS_API GraphicsDevice_CreatePipelineState(ID3D12Device* _this, ID3D12PipelineState** ppPipelineState, D3D12_INPUT_ELEMENT_DESC* pInputLayout, UINT inputLayoutCount, ID3D12RootSignature* pRootSignature,
	ID3DBlob* pVertexShader, ID3DBlob* pFragmentShader, D3D12_PRIMITIVE_TOPOLOGY_TYPE topologyType, DXGI_FORMAT renderTargetFormat, UINT msaa, UINT aaQuality, UINT sampleMask)
{
	D3D12_SHADER_BYTECODE vertexShaderBytecode = {};
	vertexShaderBytecode.pShaderBytecode = pVertexShader->GetBufferPointer();
	vertexShaderBytecode.BytecodeLength = pVertexShader->GetBufferSize();

	D3D12_SHADER_BYTECODE fragmentShaderBytecode = {};
	fragmentShaderBytecode.pShaderBytecode = pFragmentShader->GetBufferPointer();
	fragmentShaderBytecode.BytecodeLength = pFragmentShader->GetBufferSize();

	DXGI_SAMPLE_DESC sampleDesc = {};
	sampleDesc.Count = msaa;
	sampleDesc.Quality = aaQuality;

	D3D12_INPUT_LAYOUT_DESC inputLayoutDesc = {};
	inputLayoutDesc.NumElements = inputLayoutCount;
	inputLayoutDesc.pInputElementDescs = pInputLayout;

	D3D12_GRAPHICS_PIPELINE_STATE_DESC psoDesc = {};
	psoDesc.InputLayout = inputLayoutDesc;
	psoDesc.pRootSignature = pRootSignature;
	psoDesc.VS = vertexShaderBytecode;
	psoDesc.PS = fragmentShaderBytecode;
	psoDesc.PrimitiveTopologyType = topologyType;
	psoDesc.RTVFormats[0] = renderTargetFormat;
	psoDesc.SampleDesc = sampleDesc;
	psoDesc.SampleMask = sampleMask;
	psoDesc.RasterizerState = CD3DX12_RASTERIZER_DESC(D3D12_DEFAULT);
	psoDesc.BlendState = CD3DX12_BLEND_DESC(D3D12_DEFAULT);
	psoDesc.NumRenderTargets = 1;

	ID3D12PipelineState* pPipelineState;
	HRESULT hr = _this->CreateGraphicsPipelineState(&psoDesc, IID_PPV_ARGS(&pPipelineState));
	*ppPipelineState = pPipelineState;
	return hr;
}

int KAIROS_API GraphicsDevice_CreateCommittedBufferResource(ID3D12Device* _this, ID3D12Resource** ppBuffer, D3D12_HEAP_TYPE heapType, UINT64 resourceSize, D3D12_HEAP_FLAGS heapFlags, D3D12_RESOURCE_STATES resourceStates)
{
	D3D12_HEAP_PROPERTIES heapProperties = CD3DX12_HEAP_PROPERTIES(heapType);
	D3D12_RESOURCE_DESC heapDesc = CD3DX12_RESOURCE_DESC::Buffer(resourceSize);
	ID3D12Resource* pResource;
	HRESULT hr = _this->CreateCommittedResource(
		&heapProperties,
		heapFlags,
		&heapDesc,
		resourceStates,
		nullptr,
		IID_PPV_ARGS(&pResource)
	);
	*ppBuffer = pResource;
	return hr;
}

KAIROS_EXPORT_END
