import pandas as pd
import numpy as np

# CSVファイルの読み込み
odometry_file = "stats/odometry.csv"
images_file = "stats/images.csv"

odometry_data = pd.read_csv(odometry_file)
images_data = pd.read_csv(images_file)


# 通信量 (Mbps) を計算する関数
def calculate_bandwidth(data):
    # タイムスタンプの差分を計算
    data["time_diff"] = data["stamp"].diff()
    # 通信量 (Mbps) を計算: size (Bytes) -> bits / 秒 -> Mbps
    data["bandwidth_mbps"] = (data["size"] * 8) / (data["time_diff"] * 1e6)
    return data


# データに通信量を追加
odometry_data = calculate_bandwidth(odometry_data)
images_data = calculate_bandwidth(images_data)

# 通信量を足し合わせる
# オドメトリと画像のタイムスタンプを統合
combined_timestamps = np.union1d(odometry_data["stamp"], images_data["stamp"])

# 各タイムスタンプに対応する通信量を再サンプリング
odometry_resampled = pd.DataFrame({"stamp": combined_timestamps})
images_resampled = pd.DataFrame({"stamp": combined_timestamps})

odometry_resampled["bandwidth_mbps"] = np.interp(
    combined_timestamps,
    odometry_data["stamp"],
    odometry_data["bandwidth_mbps"].fillna(0),
)
images_resampled["bandwidth_mbps"] = np.interp(
    combined_timestamps, images_data["stamp"], images_data["bandwidth_mbps"].fillna(0)
)

# 合計通信量を計算
total_bandwidth = (
    odometry_resampled["bandwidth_mbps"] + images_resampled["bandwidth_mbps"]
)

# 合計通信量の平均と標準偏差
total_bandwidth_avg = total_bandwidth.mean()
total_bandwidth_std = total_bandwidth.std()

# レイテンシの平均と標準偏差 (ミリ秒単位)
odometry_latency_avg = odometry_data["latency"].mean() * 1000  # ミリ秒単位に変換
odometry_latency_std = odometry_data["latency"].std() * 1000  # ミリ秒単位に変換

images_latency_avg = images_data["latency"].mean() * 1000  # ミリ秒単位に変換
images_latency_std = images_data["latency"].std() * 1000  # ミリ秒単位に変換

# 結果の出力
print(
    "合計通信量 (Mbps) - 平均: {:.4f}, 標準偏差: {:.4f}".format(
        total_bandwidth_avg, total_bandwidth_std
    )
)
print(
    "オドメトリレイテンシ (ms) - 平均: {:.4f}, 標準偏差: {:.4f}".format(
        odometry_latency_avg, odometry_latency_std
    )
)
print(
    "画像レイテンシ (ms) - 平均: {:.4f}, 標準偏差: {:.4f}".format(
        images_latency_avg, images_latency_std
    )
)
