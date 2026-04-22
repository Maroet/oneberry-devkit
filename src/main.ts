import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHistory } from 'vue-router'
import naive from 'naive-ui'
import App from './App.vue'
import './styles/global.css'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'dashboard',
      component: () => import('./views/Dashboard.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('./views/Settings.vue'),
    },
    {
      path: '/logs',
      name: 'logs',
      component: () => import('./views/Logs.vue'),
    },
  ],
})

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(naive)
app.mount('#app')
