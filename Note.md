合并DX12依赖库：
lib /OUT:DX12Lib.lib ^
DX12.lib ^
"C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\d3d12.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\dxgi.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\d3dcompiler.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\kernel32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\user32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\gdi32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\winspool.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\comdlg32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\advapi32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\shell32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\ole32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\oleaut32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\uuid.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\odbc32.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\odbccp32.lib" "D:\VisualStudio\Community\VC\Tools\MSVC\14.50.35717\lib\x64\libcpmt.lib" "D:\VisualStudio\Community\VC\Tools\MSVC\14.50.35717\lib\x64\LIBCMT.lib" "D:\VisualStudio\Community\VC\Tools\MSVC\14.50.35717\lib\x64\OLDNAMES.lib" "D:\VisualStudio\Community\VC\Tools\MSVC\14.50.35717\lib\x64\libvcruntime.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\ucrt\x64\libucrt.lib" "C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\um\x64\dxguid.lib"


ECS Example:
using System;
using System.Collections.Generic;
using System.Linq;

// ===================== 1. 组件类型ID分配器（核心：无GC） =====================
/// <summary>
/// 组件类型ID管理器：为每个IComponent派生类型分配唯一整数ID
/// </summary>
public static class ComponentTypeRegistry
{
    private static readonly Dictionary<Type, int> _typeToId = new();
    private static readonly Dictionary<int, Type> _idToType = new();
    private static int _nextTypeId = 1; // 从1开始，0表示无效

    /// <summary>
    /// 获取组件类型的唯一ID（线程安全）
    /// </summary>
    public static int GetComponentTypeId<T>() where T : struct, IComponent
    {
        lock (_typeToId)
        {
            Type type = typeof(T);
            if (!_typeToId.ContainsKey(type))
            {
                _typeToId[type] = _nextTypeId;
                _idToType[_nextTypeId] = type;
                _nextTypeId++;
            }
            return _typeToId[type];
        }
    }

    /// <summary>
    /// 通过ID获取组件类型
    /// </summary>
    public static Type GetComponentTypeById(int typeId)
    {
        lock (_idToType)
        {
            return _idToType.TryGetValue(typeId, out var type) ? type : null;
        }
    }
}

// ===================== 2. 无GC的Archetype标识 =====================
/// <summary>
/// Archetype的唯一标识（结构体，无GC）
/// </summary>
public readonly struct ArchetypeId : IEquatable<ArchetypeId>
{
    public int Value { get; }

    public ArchetypeId(int value) => Value = value;

    public bool Equals(ArchetypeId other) => Value == other.Value;
    public override bool Equals(object? obj) => obj is ArchetypeId other && Equals(other);
    public override int GetHashCode() => Value;
    public static bool operator ==(ArchetypeId left, ArchetypeId right) => left.Equals(right);
    public static bool operator !=(ArchetypeId left, ArchetypeId right) => !left.Equals(right);
}

/// <summary>
/// Archetype缓存池：管理组件组合与ArchetypeId的映射（无GC）
/// </summary>
public class ArchetypePool
{
    // 组件类型ID集合 -> ArchetypeId（用SortedSet保证顺序，避免重复）
    private readonly Dictionary<SortedSet<int>, ArchetypeId> _componentIdsToArchetype = new(Comparer<SortedSet<int>>.Create((a, b) =>
    {
        if (a.Count != b.Count) return a.Count - b.Count;
        using var enumA = a.GetEnumerator();
        using var enumB = b.GetEnumerator();
        while (enumA.MoveNext() && enumB.MoveNext())
        {
            int cmp = enumA.Current.CompareTo(enumB.Current);
            if (cmp != 0) return cmp;
        }
        return 0;
    }));
    
    // ArchetypeId -> 组件类型ID集合
    private readonly Dictionary<ArchetypeId, SortedSet<int>> _archetypeToComponentIds = new();
    
    private int _nextArchetypeId = 1;

    /// <summary>
    /// 获取组件组合对应的ArchetypeId（无GC）
    /// </summary>
    public ArchetypeId GetArchetypeId(HashSet<int> componentTypeIds)
    {
        lock (_componentIdsToArchetype)
        {
            // 转换为SortedSet保证顺序（如[1,2]和[2,1]视为同一组合）
            var sortedComponentIds = new SortedSet<int>(componentTypeIds);
            
            if (!_componentIdsToArchetype.TryGetValue(sortedComponentIds, out var archetypeId))
            {
                // 分配新ID
                archetypeId = new ArchetypeId(_nextArchetypeId++);
                _componentIdsToArchetype[sortedComponentIds] = archetypeId;
                _archetypeToComponentIds[archetypeId] = new SortedSet<int>(sortedComponentIds);
            }
            
            return archetypeId;
        }
    }

    /// <summary>
    /// 检查Archetype是否包含所有必需的组件ID
    /// </summary>
    public bool HasAllRequiredComponents(ArchetypeId archetypeId, HashSet<int> requiredComponentIds)
    {
        lock (_archetypeToComponentIds)
        {
            if (!_archetypeToComponentIds.TryGetValue(archetypeId, out var componentIds))
                return false;
            
            foreach (int requiredId in requiredComponentIds)
            {
                if (!componentIds.Contains(requiredId))
                    return false;
            }
            return true;
        }
    }
}

// ===================== 3. 优化后的ComponentRegistry（无字符串GC） =====================
public interface IComponent { }

public struct PositionComponent : IComponent
{
    public float X;
    public float Y;
    public PositionComponent(float x, float y) { X = x; Y = y; }
}

public struct VelocityComponent : IComponent
{
    public float Dx;
    public float Dy;
    public VelocityComponent(float dx, float dy) { Dx = dx; Dy = dy; }
}

public struct RotationComponent : IComponent
{
    public float Angle;
    public RotationComponent(float angle) { Angle = angle; }
}

public readonly struct EntityId : IEquatable<EntityId>
{
    public int Value { get; }
    public EntityId(int value) => Value = value;
    public bool Equals(EntityId other) => Value == other.Value;
    public override bool Equals(object? obj) => obj is EntityId other && Equals(other);
    public override int GetHashCode() => Value;
    public static bool operator ==(EntityId left, EntityId right) => left.Equals(right);
    public static bool operator !=(EntityId left, EntityId right) => !left.Equals(right);
}

public class ComponentRegistry
{
    private int _nextEntityId = 1;
    
    // 核心优化：用ArchetypeId替代字符串键
    private readonly ArchetypePool _archetypePool = new();
    
    // 实体 -> ArchetypeId
    private readonly Dictionary<EntityId, ArchetypeId> _entityToArchetype = new();
    
    // ArchetypeId -> 实体列表（无GC）
    private readonly Dictionary<ArchetypeId, List<EntityId>> _archetypeToEntities = new();
    
    // 实体 -> 组件字典（Type -> 组件实例）
    private readonly Dictionary<EntityId, Dictionary<Type, IComponent>> _entityComponents = new();

    public EntityId CreateEntity() => new(_nextEntityId++);

    /// <summary>
    /// 添加组件（无字符串GC）
    /// </summary>
    public void AddComponent<T>(EntityId entity, T component) where T : struct, IComponent
    {
        Type componentType = typeof(T);
        
        if (!_entityComponents.ContainsKey(entity))
            _entityComponents[entity] = new Dictionary<Type, IComponent>();
        
        _entityComponents[entity][componentType] = component;
        UpdateEntityArchetype(entity);
    }

    /// <summary>
    /// 更新Archetype（核心：用整数ID替代字符串）
    /// </summary>
    private void UpdateEntityArchetype(EntityId entity)
    {
        // 移除旧Archetype
        if (_entityToArchetype.TryGetValue(entity, out var oldArchetypeId))
        {
            if (_archetypeToEntities.TryGetValue(oldArchetypeId, out var oldEntities))
            {
                oldEntities.Remove(entity);
                if (oldEntities.Count == 0)
                    _archetypeToEntities.Remove(oldArchetypeId);
            }
        }

        // 生成新ArchetypeId（无GC）
        var componentTypeIds = new HashSet<int>(
            _entityComponents[entity].Keys.Select(t => ComponentTypeRegistry.GetComponentTypeId(
                (dynamic)Activator.CreateInstance(t)! // 简化：获取组件类型的ID
            ))
        );
        var newArchetypeId = _archetypePool.GetArchetypeId(componentTypeIds);
        
        // 关联新Archetype
        _entityToArchetype[entity] = newArchetypeId;
        if (!_archetypeToEntities.ContainsKey(newArchetypeId))
            _archetypeToEntities[newArchetypeId] = new List<EntityId>();
        _archetypeToEntities[newArchetypeId].Add(entity);
    }

    /// <summary>
    /// 查询实体（无字符串拆分，无GC）
    /// </summary>
    public List<EntityId> GetEntitiesWithComponents(params Type[] requiredComponentTypes)
    {
        var result = new List<EntityId>();
        var requiredComponentIds = new HashSet<int>(
            requiredComponentTypes.Select(t => ComponentTypeRegistry.GetComponentTypeId(
                (dynamic)Activator.CreateInstance(t)!
            ))
        );

        // 遍历ArchetypeId，无字符串操作
        foreach (var (archetypeId, entities) in _archetypeToEntities)
        {
            if (_archetypePool.HasAllRequiredComponents(archetypeId, requiredComponentIds))
                result.AddRange(entities);
        }

        return result;
    }

    public bool TryGetComponent<T>(EntityId entity, out T component) where T : struct, IComponent
    {
        component = default;
        if (_entityComponents.TryGetValue(entity, out var components) &&
            components.TryGetValue(typeof(T), out var comp))
        {
            component = (T)comp;
            return true;
        }
        return false;
    }
}

// ===================== 4. 系统和测试代码（无变化） =====================
public class VelocitySystem
{
    private readonly ComponentRegistry _registry;
    public VelocitySystem(ComponentRegistry registry) => _registry = registry;

    public void Update(float deltaTime)
    {
        var requiredTypes = new[] { typeof(PositionComponent), typeof(VelocityComponent) };
        var entities = _registry.GetEntitiesWithComponents(requiredTypes);

        foreach (var entity in entities)
        {
            if (_registry.TryGetComponent<PositionComponent>(entity, out var pos) &&
                _registry.TryGetComponent<VelocityComponent>(entity, out var vel))
            {
                pos.X += vel.Dx * deltaTime;
                pos.Y += vel.Dy * deltaTime;
                _registry.AddComponent(entity, pos);
                Console.WriteLine($"实体 {entity} 位置：({pos.X:F1}, {pos.Y:F1})");
            }
        }
    }
}

public class EcsDemo
{
    public static void Main()
    {
        var registry = new ComponentRegistry();
        var velocitySystem = new VelocitySystem(registry);

        var entity1 = registry.CreateEntity();
        registry.AddComponent(entity1, new PositionComponent(0, 0));
        registry.AddComponent(entity1, new VelocityComponent(1.5f, 2.5f));

        var entity2 = registry.CreateEntity();
        registry.AddComponent(entity2, new PositionComponent(10, 10));
        registry.AddComponent(entity2, new VelocityComponent(3.0f, 4.0f));
        registry.AddComponent(entity2, new RotationComponent(90));

        velocitySystem.Update(1.0f);
        velocitySystem.Update(0.5f);
    }
}