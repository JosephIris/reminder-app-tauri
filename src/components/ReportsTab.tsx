import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from "chart.js";
import { Line, Bar, Doughnut } from "react-chartjs-2";

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  ArcElement,
  Title,
  Tooltip,
  Legend,
  Filler
);

interface HistoricalStats {
  dailyCompletions: [string, number][];
  hourlyDistribution: number[];
  dailyDistribution: number[];
  backlogSize: number;
}

const DAYS_OF_WEEK = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

export function ReportsTab() {
  const [stats, setStats] = useState<HistoricalStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadStats();
  }, []);

  const loadStats = async () => {
    try {
      const result = await invoke<[Array<[string, number]>, number[], number[], number]>(
        "get_historical_stats"
      );
      setStats({
        dailyCompletions: result[0],
        hourlyDistribution: result[1],
        dailyDistribution: result[2],
        backlogSize: result[3],
      });
    } catch (e) {
      console.error("Failed to load historical stats:", e);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-gray-400 animate-pulse-soft">Loading stats...</div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-gray-400">Unable to load stats</div>
      </div>
    );
  }

  // Calculate summary stats
  const totalCompleted = stats.dailyCompletions.reduce((sum, [, count]) => sum + count, 0);
  const avgPerDay = totalCompleted / 14;
  const peakHour = stats.hourlyDistribution.indexOf(Math.max(...stats.hourlyDistribution));
  const peakDay = stats.dailyDistribution.indexOf(Math.max(...stats.dailyDistribution));

  // Daily completions chart data
  const dailyChartData = {
    labels: stats.dailyCompletions.map(([date]) => {
      const d = new Date(date);
      return `${d.getMonth() + 1}/${d.getDate()}`;
    }),
    datasets: [
      {
        label: "Tasks Completed",
        data: stats.dailyCompletions.map(([, count]) => count),
        borderColor: "#60a5fa",
        backgroundColor: "rgba(96, 165, 250, 0.1)",
        fill: true,
        tension: 0.3,
        pointRadius: 3,
        pointBackgroundColor: "#60a5fa",
      },
    ],
  };

  // Hourly distribution chart data
  const hourlyChartData = {
    labels: Array.from({ length: 24 }, (_, i) => {
      if (i === 0) return "12a";
      if (i === 12) return "12p";
      return i < 12 ? `${i}a` : `${i - 12}p`;
    }),
    datasets: [
      {
        label: "Tasks Completed",
        data: stats.hourlyDistribution,
        backgroundColor: "rgba(34, 197, 94, 0.6)",
        borderColor: "#22c55e",
        borderWidth: 1,
      },
    ],
  };

  // Daily distribution chart data
  const weeklyChartData = {
    labels: DAYS_OF_WEEK,
    datasets: [
      {
        label: "Tasks Completed",
        data: stats.dailyDistribution,
        backgroundColor: "rgba(168, 85, 247, 0.6)",
        borderColor: "#a855f7",
        borderWidth: 1,
      },
    ],
  };

  // Completion doughnut (actual vs backlog ratio)
  const doughnutData = {
    labels: ["Completed (14d)", "In Backlog"],
    datasets: [
      {
        data: [totalCompleted, stats.backlogSize],
        backgroundColor: ["rgba(34, 197, 94, 0.8)", "rgba(107, 114, 128, 0.8)"],
        borderColor: ["#22c55e", "#6b7280"],
        borderWidth: 2,
      },
    ],
  };

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        display: false,
      },
    },
    scales: {
      x: {
        grid: {
          color: "rgba(75, 85, 99, 0.3)",
        },
        ticks: {
          color: "#9ca3af",
          font: { size: 10 },
        },
      },
      y: {
        beginAtZero: true,
        grid: {
          color: "rgba(75, 85, 99, 0.3)",
        },
        ticks: {
          color: "#9ca3af",
          font: { size: 10 },
          stepSize: 1,
        },
      },
    },
  };

  const doughnutOptions = {
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: {
        position: "bottom" as const,
        labels: {
          color: "#9ca3af",
          font: { size: 11 },
          padding: 10,
        },
      },
    },
  };

  return (
    <div className="flex-1 overflow-y-auto space-y-4">
      {/* Summary cards */}
      <div className="grid grid-cols-4 gap-3">
        <div className="bg-dark-700 rounded-lg p-3 text-center">
          <div className="text-2xl font-bold text-accent-green">{totalCompleted}</div>
          <div className="text-xs text-gray-500">Past 14 Days</div>
        </div>
        <div className="bg-dark-700 rounded-lg p-3 text-center">
          <div className="text-2xl font-bold text-accent-blue">{avgPerDay.toFixed(1)}</div>
          <div className="text-xs text-gray-500">Avg Per Day</div>
        </div>
        <div className="bg-dark-700 rounded-lg p-3 text-center">
          <div className="text-2xl font-bold text-purple-400">{DAYS_OF_WEEK[peakDay]}</div>
          <div className="text-xs text-gray-500">Most Productive Day</div>
        </div>
        <div className="bg-dark-700 rounded-lg p-3 text-center">
          <div className="text-2xl font-bold text-orange-400">
            {peakHour > 12 ? `${peakHour - 12}pm` : peakHour === 0 ? "12am" : `${peakHour}am`}
          </div>
          <div className="text-xs text-gray-500">Peak Hour</div>
        </div>
      </div>

      {/* Throughput chart */}
      <div className="bg-dark-700 rounded-lg p-4">
        <h3 className="text-sm font-medium text-gray-400 mb-3">Daily Completions (Past 2 Weeks)</h3>
        <div className="h-40">
          <Line data={dailyChartData} options={chartOptions} />
        </div>
      </div>

      {/* Two column layout for smaller charts */}
      <div className="grid grid-cols-2 gap-4">
        {/* Hourly distribution */}
        <div className="bg-dark-700 rounded-lg p-4">
          <h3 className="text-sm font-medium text-gray-400 mb-3">By Hour of Day</h3>
          <div className="h-32">
            <Bar data={hourlyChartData} options={chartOptions} />
          </div>
        </div>

        {/* Weekly distribution */}
        <div className="bg-dark-700 rounded-lg p-4">
          <h3 className="text-sm font-medium text-gray-400 mb-3">By Day of Week</h3>
          <div className="h-32">
            <Bar data={weeklyChartData} options={chartOptions} />
          </div>
        </div>
      </div>

      {/* Progress doughnut */}
      <div className="bg-dark-700 rounded-lg p-4">
        <h3 className="text-sm font-medium text-gray-400 mb-3">Completed vs Backlog</h3>
        <div className="h-40 flex justify-center">
          <div className="w-40">
            <Doughnut data={doughnutData} options={doughnutOptions} />
          </div>
        </div>
      </div>
    </div>
  );
}
