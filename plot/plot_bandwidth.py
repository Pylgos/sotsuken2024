import pandas as pd
import matplotlib.pyplot as plt
from matplotlib import rcParams
import numpy as np

# 日本語フォントの設定
rcParams["font.family"] = "Noto Sans CJK JP"  # 環境に応じてフォント名を調整してください
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

# タイムスタンプの基準を画像データの2つ目のタイムスタンプに調整
start_time = images_data["stamp"].iloc[1]  # 2つ目のタイムスタンプ
odometry_data["stamp"] -= start_time
images_data["stamp"] -= start_time

# タイムスタンプを基準に統合
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

# プロットの準備
plt.figure(figsize=(10, 6))

# 個別の通信量をプロット
plt.plot(
    odometry_resampled["stamp"],
    odometry_resampled["bandwidth_mbps"],
    label="Odometry Bandwidth (Mbps)",
    color="blue",
    linestyle="--",
)

plt.plot(
    images_resampled["stamp"],
    images_resampled["bandwidth_mbps"],
    label="Images Bandwidth (Mbps)",
    color="orange",
    linestyle="--",
)

# 合計通信量をプロット
plt.plot(
    combined_timestamps,
    total_bandwidth,
    label="Total Bandwidth (Mbps)",
    color="green",
    linewidth=2,
)

# グラフの装飾
plt.title("通信量", fontsize=20, fontweight="bold")
plt.xlabel("時間 [s]")
plt.ylabel("帯域幅 [Mbps]")
plt.legend()
plt.grid(True)

# 軸の範囲を調整（横軸をデータ範囲にピッタリ合わせる）
plt.xlim(0, combined_timestamps.max())  # 横軸: 0秒から最大値
plt.ylim(
    0,
    2.5,
    # total_bandwidth.max(),
)

# グラフの表示
plt.tight_layout()
plt.show()
