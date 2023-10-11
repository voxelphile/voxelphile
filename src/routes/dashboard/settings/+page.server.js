/** @type {import('./$types').Actions} */
import { error } from "@sveltejs/kit";
import { api } from "../../../const.js";
import { fetch_promise } from "../../../user-form.js";


export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();    

        {
            const username = formData.get('username')?.toString();        
            const email = formData.get('email')?.toString();        
            const password = formData.get('password')?.toString();
            const repassword = formData.get('repassword').toString();

            let errors = {};

            if(password != '' && repassword != '' && password != repassword) {
                errors["repassword_error"] = 'Must match';
            }
            if(password != '' && password.toString().length < 6) {
                errors["password_error"] = 'Must be at least 6 characters';
            }
            if(password != '' && password.toString().length > 128) {
                errors["password_error"]= 'Must be at most 128 characters';
            }
            if(username != null && !username.toString().match(/^[0-9a-zA-Z]+$/)) {
                errors["username_error"]= 'Must be alphanumeric';
            }
            if(username == null || username != null && username.toString() == '') {
                errors["username_error"]= 'Cannot be empty';
            }
            if(username != null && username.toString().length > 32) {
                errors["username_error"]= 'Must be at most 32 characters';
            }
            if(email == null || email != null && email.toString() == '') {
                errors["email_error"]= 'Cannot be empty';
            }
            if(email != null && email.indexOf("@") == -1) {
                errors["email_error"]= 'Must contain an @ symbol';
            }
            if(email != null && email.indexOf("@") == email.length - 1) {
                errors["email_error"]= 'Must contain a part after the @ symbol';
            }
            
            if (Object.keys(errors).length > 0) {
                return errors;
            }
        }
        
        let json = { };
        
        if (formData.get('profile') != null) {
            console.log("formdata yo");
            var base64regex = /^([0-9a-zA-Z+/]{4})*(([0-9a-zA-Z+/]{2}==)|([0-9a-zA-Z+/]{3}=))?$/;
            let encoding = "data:image/jpeg;base64,";
            let data = formData.get('profile')?.toString();
            let base64_data = data.slice(0).replace(encoding, "");
            if (data.includes(encoding) && base64regex.test(base64_data)) {
                json['profile'] = formData.get('profile').toString();
            }
        }

        if (formData.get('email') != null) {
            if (formData.get('email')?.toString() != '') {
                json['email'] = formData.get('email')?.toString();
            }
        }
        if (formData.get('username') != null ) {
            if (formData.get('username')?.toString() != '') {
                json['username'] = formData.get('username')?.toString();
            }
        }

        if (formData.get('password') != null && formData.get('password') == formData.get('repassword')
        ) {
            if (formData.get('password')?.toString() != '') {
                json['password'] = formData.get('password')?.toString();
            }
        }

        const request = new Request(api + "/user/change", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': 'Bearer ' + event.cookies.get("jwt")
            },
            body: JSON.stringify(json),
        });
        
        let response = await fetch(request).catch((response) => {
            throw error(response?.status);
        });
    
        if (response?.status != 200) {
            throw error(response?.status);
        }

        return { success: true };
	}
};