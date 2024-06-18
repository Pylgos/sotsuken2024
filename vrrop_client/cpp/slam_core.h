#ifdef __cplusplus
extern "C" {
#endif

typedef void* slam_core_ptr_t;

slam_core_ptr_t slam_core_create();
void slam_core_delete(slam_core_ptr_t p);

#ifdef __cplusplus
}
#endif
