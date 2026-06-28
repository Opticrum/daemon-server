import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/dashboard',
  },
  {
    path: '/dashboard',
    name: 'Dashboard',
    component: () => import('@/views/DashboardView.vue'),
  },
  {
    path: '/wallets',
    name: 'Wallets',
    component: () => import('@/views/WalletsView.vue'),
  },
  {
    path: '/orders',
    name: 'Orders',
    component: () => import('@/views/OrdersView.vue'),
  },
  {
    path: '/matches',
    name: 'Matches',
    component: () => import('@/views/MatchesView.vue'),
  },
  {
    path: '/channels',
    name: 'Channels',
    component: () => import('@/views/ChannelsView.vue'),
  },
  {
    path: '/settings',
    name: 'Settings',
    component: () => import('@/views/SettingsView.vue'),
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

export default router
