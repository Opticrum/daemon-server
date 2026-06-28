import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import {
  Chart,
  ArcElement,
  Tooltip,
  Legend,
  DoughnutController,
  LineController,
  BarController,
  LineElement,
  BarElement,
  PointElement,
  CategoryScale,
  LinearScale,
  Filler,
} from 'chart.js'
import './styles/variables.css'
import './styles/global.css'

Chart.register(
  ArcElement,
  Tooltip,
  Legend,
  DoughnutController,
  LineController,
  BarController,
  LineElement,
  BarElement,
  PointElement,
  CategoryScale,
  LinearScale,
  Filler,
)

const app = createApp(App)
app.use(router)
app.mount('#app')
