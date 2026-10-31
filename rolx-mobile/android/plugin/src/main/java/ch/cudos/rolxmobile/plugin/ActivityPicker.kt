package ch.cudos.rolxmobile.plugin

import android.app.Activity
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.TextWatcher
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.EditText
import android.widget.ListView
import android.widget.TextView
import android.widget.Toast
import java.util.concurrent.Executors
import org.json.JSONArray
import org.json.JSONObject

/**
 * The widget's activity picker, a small dialog over the home screen: favourites and recently used activities first,
 * a search field for the rest (ranked by the Rust library like the app's picker). Picking one stamps in on it, or
 * continues on it when stamped in (`RolxNative.pick`). Not exported: only the widget opens it.
 */
class ActivityPicker : Activity() {
    private val worker = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private val adapter = ItemAdapter()

    /** Counts the searches, so a slow answer to an old query does not replace a newer one. */
    private var generation = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_picker)
        val list = findViewById<ListView>(R.id.picker_list)
        list.adapter = adapter
        list.emptyView = findViewById(R.id.picker_empty)
        list.setOnItemClickListener { _, _, position, _ -> pick(adapter.getItem(position)) }
        findViewById<EditText>(R.id.picker_search).addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
            override fun afterTextChanged(s: Editable?) = search(s?.toString() ?: "")
        })
        search("")
    }

    override fun onDestroy() {
        worker.shutdown()
        super.onDestroy()
    }

    private fun search(query: String) {
        val mine = ++generation
        val app = applicationContext
        worker.execute {
            val result = try {
                JSONObject(RolxNative.searchActivities(app, query))
            } catch (e: Throwable) {
                Log.w(TAG, "cannot list the activities", e)
                null
            }
            main.post {
                if (mine != generation || isFinishing) return@post
                val stampedIn = result?.optBoolean("stampedIn") ?: false
                findViewById<TextView>(R.id.picker_title).setText(
                    if (stampedIn) R.string.picker_title_switch else R.string.picker_title_stamp_in,
                )
                adapter.show(result?.optJSONArray("items") ?: JSONArray())
            }
        }
    }

    private fun pick(item: JSONObject) {
        val app = applicationContext
        val id = item.getInt("id")
        worker.execute {
            val result = try {
                JSONObject(RolxNative.pick(app, id))
            } catch (e: Throwable) {
                Log.w(TAG, "picking an activity failed", e)
                JSONObject().put("result", "error").put("message", e.toString())
            }
            if (result.optBoolean("upload")) {
                BackgroundUpload.schedule(app)
            }
            StampWidget.refreshAll(app)
            main.post {
                when (result.optString("result")) {
                    "stale" -> {
                        toast("Open RolX to end the forgotten session")
                        StampWidget.openAppIntent(app).send()
                    }
                    "error" -> toast("Could not stamp: ${result.optString("message")}")
                }
                finish()
            }
        }
    }

    private fun toast(text: String) = Toast.makeText(applicationContext, text, Toast.LENGTH_LONG).show()

    /** Rows of two lines: the short name (with "running" or the group), and the full label. */
    private inner class ItemAdapter : BaseAdapter() {
        private var items = JSONArray()

        fun show(items: JSONArray) {
            this.items = items
            notifyDataSetChanged()
        }

        override fun getCount() = items.length()
        override fun getItem(position: Int): JSONObject = items.getJSONObject(position)
        override fun getItemId(position: Int) = getItem(position).getLong("id")

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val view = convertView ?: layoutInflater.inflate(R.layout.picker_item, parent, false)
            val item = getItem(position)
            val tag = when {
                item.optBoolean("current") -> getString(R.string.picker_running)
                item.optString("group") == "favourite" -> "★"
                item.optString("group") == "recent" -> getString(R.string.picker_recent)
                else -> ""
            }
            view.findViewById<TextView>(R.id.picker_item_short).text = item.optString("short")
            view.findViewById<TextView>(R.id.picker_item_tag).text = tag
            view.findViewById<TextView>(R.id.picker_item_label).text = item.optString("label")
            return view
        }
    }

    private companion object {
        const val TAG = "RolxPicker"
    }
}
