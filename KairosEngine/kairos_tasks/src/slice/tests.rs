use crate::{TaskPool, slice::{ParallelSlice, ParallelSliceMut}};

#[test]
fn test_par_chunks_map() {
    let v = vec![42; 1000];
    let task_pool = TaskPool::new();
    let outputs = v.par_splat_map(&task_pool, None, |_, numbers| -> i32 {
        numbers.iter().sum()
    });

    let mut sum = 0;
    for output in outputs {
        sum += output;
    }

    assert_eq!(sum, 1000 * 42);
}

#[test]
fn test_par_chunks_map_mut() {
    let mut v = vec![42; 1000];
    let task_pool = TaskPool::new();

    let outputs = v.par_splat_map_mut(&task_pool, None, |_, numbers| -> i32 {
        for number in numbers.iter_mut() {
            *number *= 2;
        }
        numbers.iter().sum()
    });

    let mut sum = 0;
    for output in outputs {
        sum += output;
    }

    assert_eq!(sum, 1000 * 42 * 2);
    assert_eq!(v[0], 84);
}

#[test]
fn test_par_chunks_map_index() {
    let v = vec![1; 1000];
    let task_pool = TaskPool::new();
    let outputs = v.par_chunk_map(&task_pool, 100, |index, numbers| -> i32 {
        numbers.iter().sum::<i32>() * index as i32
    });

    assert_eq!(outputs.iter().sum::<i32>(), 100 * (9 * 10) / 2);
}
